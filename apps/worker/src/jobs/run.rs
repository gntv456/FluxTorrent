//! run_all 总调度 + with_lock + module_on + 计费乘数 + 单测。
//! 从 jobs.rs 按域拆出。

use super::*;
use sqlx::PgPool;

/// 角色分片（0225 G30-B1）：FLUX_WORKER_JOBS=逗号分隔 job 名（无 job: 前缀）。
/// 非空时本实例只跑清单内的定时 job（手动触发认领不受限——面板操作应总有实例接）。
/// 空缺省 = 全量（单实例行为不变）。示例：FLUX_WORKER_JOBS=consume_announce,
/// consume_agent_blocks 让一台专吃计费流，另一台跑其余。
fn job_shard() -> Option<Vec<String>> {
    let v = std::env::var("FLUX_WORKER_JOBS").unwrap_or_default();
    let list: Vec<String> = v
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

/// 分片内才执行（with_lock 包装口）。
macro_rules! shard_lock {
    ($db:expr, $key:literal, $fut:expr, $shard:expr) => {
        if let Some(list) = $shard {
            let name = $key.trim_start_matches("job:");
            if !list.iter().any(|s| s == name) {
                continue;
            }
        }
        with_lock($db, $key, $fut).await;
    };
}

/// 分片判定（spawn 前置过滤，与 shard_lock! 同口径）
fn job_in_shard(name: &str, shard: &Option<Vec<String>>) -> bool {
    match shard {
        Some(list) => list.iter().any(|s| s == name),
        None => true,
    }
}

pub async fn run_all(
    db: PgPool,
    redis: redis::aio::ConnectionManager,
) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut hour_tick =
        tokio::time::interval(std::time::Duration::from_secs(3600));
    let mut last_bank_day: Option<chrono::NaiveDate> = None;
    // 0071 反作弊/性能调度
    let mut tick10 = tokio::time::interval(std::time::Duration::from_secs(600));
    let mut tick30 =
        tokio::time::interval(std::time::Duration::from_secs(1800));
    let mut tick6h =
        tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
    let mut tick1d =
        tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
    let mut first_tick10 = true;
    let mut first_tick30 = true;
    let mut first_tick6h = true;
    let mut first_tick1d = true;
    // 手动任务句柄收口（0224 G30）：停机时 drain 等待在跑任务，而非裸 spawn 弃置
    let mut manual_tasks = crate::shutdown::TaskSet::new();
    let shard = job_shard();
    if let Some(list) = &shard {
        tracing::info!(jobs = list.join(","), "角色分片生效（仅清单内定时 job）");
    }
    sync_job_catalog(&db).await; // 目录同步（0218 G7）：面板以 job_status 为准
    loop {
        if crate::shutdown::shutting_down() {
            tracing::info!("停机：调度循环退出，排空在跑手动任务（≤60s）");
            break;
        }
        tokio::select! {
            _ = tick.tick() => {
                // 手动触发认领（0218 G7）：与定时同函数、同 advisory 锁，不阻塞本循环
                poll_manual_triggers(&db, &redis, &mut manual_tasks).await;
                // 审计修复（多实例互斥 + 超时）：每个 job 包 advisory lock + 900s 超时。
                // 多 worker 部署时同 job 只有抢到锁的实例执行（拿不到锁静默跳过本轮）；
                // 卡死任务 15 分钟后被 timeout 掐掉、连接归还，不会拖垮整个调度循环。
                // 失败/超时在 with_lock 内统一记日志（含 key），此处无需再逐个 match。
                shard_lock!(&db, "job:expire_promotions",
                    expire_promotions(&db), &shard);
                shard_lock!(&db, "job:magic_pool_promo",
                    magic_pool_promo(&db), &shard);
                shard_lock!(&db, "job:preserve_exit",
                    preserve_exit(&db), &shard);
                // consume_* 已改消费者组（0225 G30-B2）：组内投递唯一，多实例
                // 并发消费是真分摊而非双计——不再需要 advisory 锁互斥（保留
                // mark_start 登记则由面板覆盖写口径承担，executed_by 可见分实例）。
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    crate::jobs::locks::mark_start(&db,
                        "consume_announce").await;
                    if let Err(e) = consume_announce(&db2, &mut r).await {
                        tracing::error!(?e, "consume_announce 失败");
                    }
                    crate::jobs::locks::mark_end(&db, "consume_announce",
                        None).await;
                }
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    crate::jobs::locks::mark_start(&db,
                        "consume_agent_blocks").await;
                    if let Err(e) = consume_agent_blocks(&db2, &mut r).await {
                        tracing::error!(?e, "consume_agent_blocks 失败");
                    }
                    crate::jobs::locks::mark_end(&db, "consume_agent_blocks",
                        None).await;
                }
                // 0225 G30-B14：独立任务并发执行——900s 慢任务不再堵住同轮
                // 后续；锁/超时/分片语义不变。无顺序依赖（结算链在 hour 串行）。
                let mut js = tokio::task::JoinSet::new();
                if job_in_shard("backfill_pieces_hash", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:backfill_pieces_hash", backfill_pieces_hash(&d)).await;
                    });
                }
                if job_in_shard("sweep_stale_peers", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:sweep_stale_peers", sweep_stale_peers(&d)).await;
                    });
                }
                if job_in_shard("collect_milestones", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:collect_milestones", collect_milestones(&d)).await;
                    });
                }
                if job_in_shard("hr_enforce", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:hr_enforce", hr_enforce(&d)).await;
                    });
                }
                if job_in_shard("hr_punish", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:hr_punish", hr_punish(&d)).await;
                    });
                }
                if job_in_shard("class_auto_adjust", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:class_auto_adjust", class_auto_adjust(&d)).await;
                    });
                }
                if job_in_shard("preserve_seed", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:preserve_seed", preserve_seed(&d)).await;
                    });
                }
                if job_in_shard("task_settle", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:task_settle", crate::task_jobs::task_settle(&d)).await;
                    });
                }
                if job_in_shard("exam_assign", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:exam_assign", crate::task_jobs::exam_assign(&d)).await;
                    });
                }
                if job_in_shard("lottery_settle", &shard) {
                    let d = db.clone();
                    js.spawn(async move {
                        with_lock(&d, "job:lottery_settle", lottery_settle_due(&d)).await;
                    });
                }
                while let Some(res) = js.join_next().await {
                    let _ = res.map_err(|e| {
                        tracing::error!(?e, "并发 job join 失败");
                    });
                }
                // 银行结算：站点时区 UTC+8 自然日切换后跑一次；分钟级检查保证 worker 重启/宕机跨日也能补跑
                let site_day = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
                if last_bank_day.is_none() {
                    last_bank_day = Some(init_bank_day(&db).await);
                }
                if last_bank_day != Some(site_day) {
                    tracing::info!(?site_day, "bank_daily start");
                    // bank_daily 内部各子步骤自带日期游标/幂等键，锁内重跑安全
                    let db2 = db.clone();
                    with_lock(&db, "job:bank_daily", async move {
                        crate::bank_jobs::bank_daily(&db2).await;
                        Ok::<(), anyhow::Error>(())
                    })
                    .await;
                    // 结算失败（游标未写）时保持 last_bank_day 落后，下一分钟 tick 重试整轮；
                    // 成功时以 bank_settle_runs 的 run_date 为准，避免与库内游标漂移。
                    let after: Option<chrono::NaiveDate> =
                                                sqlx::query_scalar("SELECT \
                         max(run_date) FROM bank_settle_runs")
                            .fetch_one(&db)
                            .await
                            .ok()
                            .flatten();
                    last_bank_day = Some(after.unwrap_or(site_day - chrono::Duration::days(1)));
                }
            }
            _ = hour_tick.tick() => {
                // 审计修复：去掉 first_hour 首轮跳过——原逻辑为防启动风暴，但 worker
                // 频繁重启（崩溃循环/滚动发布）时 hour_interval 每次都从第一 tick 起步，
                // seeding_reward 可能数小时不被执行。本任务幂等键 = seeding:{user}:{yyyymmddhh}，
                // 同小时重复执行零副作用；其余 hourly 任务也都自带幂等护栏，首轮直接跑安全。
                // 时魔参数（底薪/封顶/曲线/体积基准/稀有度/标定）全在 site_settings `seeding_*`，
                // 由 DB 函数 seeding_params() 统一读取 —— 调价不再改代码重发（迁移 0133）。
                // 结算前先拿僵尸阈值传给结算 SQL（同 tick 内不依赖 sweep_stale_peers 是否跑过）
                let stale_secs = stale_peer_threshold_secs(&db).await;
                shard_lock!(&db, "job:seeding_reward",
                    seeding_reward(&db, stale_secs), &shard);
                shard_lock!(&db, "job:purge_old_login_events",
                    purge_old_login_events(&db), &shard);
                shard_lock!(&db, "job:ratio_watch", ratio_watch(&db), &shard);
                shard_lock!(&db, "job:dormant_mark", dormant_mark(&db), &shard);
                shard_lock!(&db, "job:wishlist_notify",
                    wishlist_notify(&db), &shard);
                shard_lock!(&db, "job:highspeed_tag",
                    highspeed_tag(&db), &shard);
                shard_lock!(&db, "job:resurrection_settle",
                    resurrection_settle(&db), &shard);
                shard_lock!(&db, "job:social_team_settle",
                    social_team_settle(&db), &shard);
                // 失败判定必须排在成功结算之后：恰好在期限内达标的队伍应算成功
                shard_lock!(&db, "job:social_team_expire",
                    social_team_expire(&db), &shard);
                // 绩效考核月末结算（0106）：挂 hourly 而非 daily——daily tick 首轮被
                // first_tick1d 跳过、重启后要等 24h 才首跑；hourly 首轮立即执行，
                // 且本 job 幂等（settled_at 标记 + 发薪幂等键），空扫描是一次索引查询
                shard_lock!(&db, "job:jixiao_settle",
                    jixiao_settle(&db), &shard);
                shard_lock!(&db, "job:preserve_settle",
                    preserve_settle(&db), &shard);
                shard_lock!(&db, "job:funding_settle",
                    funding_settle(&db), &shard);
                shard_lock!(&db, "job:refundable_settle",
                    refundable_settle(&db), &shard);
                shard_lock!(&db, "job:achievement_grant",
                    achievement_grant(&db), &shard);
                // 0148 字幕工作流：认领超时回池 + 交稿超时自动验收 + 月度评选候选
                // 0149 认证字幕人：三阈值复扫（均幂等：CAS/UNIQUE/PK + 仅撤 auto 行）
                shard_lock!(&db, "job:subreq_sweep", subreq_sweep(&db), &shard);
                shard_lock!(&db, "job:subawards", subawards_build(&db), &shard);
                shard_lock!(&db, "job:subcert_sweep",
                    subcert_sweep(&db), &shard);
                // 卫生清理（NP docleanup 口径）：过期邀请落库回收 / 一次性凭证与重置 token 清理
                shard_lock!(&db, "job:expire_invites",
                    expire_invites(&db), &shard);
                shard_lock!(&db, "job:purge_expired_tokens",
                    purge_expired_tokens(&db), &shard);
                // 运行日志保留期（0218 G6）：14 天 / 20 万行硬顶
                shard_lock!(&db, "job:purge_runtime_logs",
                    purge_runtime_logs(&db), &shard);
                // DLQ 可见性：只进不出等于变相丢计费——有积压时通知管理组信箱
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:dlq_watch", async move {
                        dlq_watch(&db2, &mut r).await
                    })
                    .await;
                }
            }
            _ = tick10.tick() => {
                if first_tick10 { first_tick10 = false; continue; }
                shard_lock!(&db, "job:cheat_audit", cheat_audit(&db), &shard);
            }
            _ = tick30.tick() => {
                if first_tick30 { first_tick30 = false; continue; }
                shard_lock!(&db, "job:multi_ip_check",
                    multi_ip_check(&db), &shard);
                shard_lock!(&db, "job:leak_scan", leak_scan(&db), &shard);
            }
            _ = tick6h.tick() => {
                if first_tick6h { first_tick6h = false; continue; }
                // 对账告警必须先于 reconcile_snapshots：收敛会抹掉差异证据
                shard_lock!(&db, "job:reconcile_diff_alert",
                    reconcile_diff_alert(&db), &shard);
                shard_lock!(&db, "job:reconcile_snapshots",
                    reconcile_snapshots(&db), &shard);
            }
            _ = tick1d.tick() => {
                if first_tick1d { first_tick1d = false; continue; }
                shard_lock!(&db, "job:ensure_partitions",
                    ensure_partitions(&db), &shard);
            }
        }
    }
    manual_tasks.drain().await;
    tracing::info!("flux-worker 已排空，退出");
    Ok(())
}

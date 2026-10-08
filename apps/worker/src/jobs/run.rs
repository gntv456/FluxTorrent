//! run_all 总调度 + with_lock + module_on + 计费乘数 + 单测。
//! 从 jobs.rs 按域拆出。

use super::*;
use sqlx::PgPool;

/// 角色分片（0225 G30-B1）：FLUX_WORKER_JOBS=逗号分隔 job 名；空缺省=全量。
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

/// 六轮审计 P1-C：select 调度循环的分支内**只允许 spawn**——任何同步 await
/// 的慢任务都会拖死同循环里的计费消费（consume_announce，revenue path），
/// 实测 collusion_check 慢查询曾让计费停摆 20 分钟。重活一律走本 helper。
fn spawn_heavy<F>(db: &PgPool, shard: &Option<Vec<String>>, reg: F)
where
    F: FnOnce(
            &mut tokio::task::JoinSet<()>,
            &PgPool,
            &Option<Vec<String>>,
        ) + Send
        + 'static,
{
    let (db2, shard2) = (db.clone(), shard.clone());
    tokio::spawn(async move {
        let mut js = tokio::task::JoinSet::new();
        reg(&mut js, &db2, &shard2);
        while let Some(res) = js.join_next().await {
            let _ = res.map_err(|e| tracing::error!(?e, "join 失败"));
        }
    });
}

/// 同上，但丢进 JoinSet 并发跑（60s 分支：900s 慢任务不堵同轮）。
/// 传函数名而非调用式：克隆出的池在宏内，调用式里的变量过不卫生检查。
macro_rules! spawn_lock {
    ($js:expr, $db:expr, $key:literal, $fut:path, $shard:expr) => {
        if job_in_shard($key.trim_start_matches("job:"), $shard) {
            let d = $db.clone();
            $js.spawn(async move {
                with_lock(&d, $key, $fut(&d)).await;
            });
        }
    };
}

fn job_in_shard(n: &str, shard: &Option<Vec<String>>) -> bool {
    shard.as_ref().map_or(true, |l| l.iter().any(|s| s == n))
}
/// 统一 Skip 语义的 interval：tick 体耗时超周期时**跳过**积压轮次，而不是默认的
/// Burst（立刻补跑）——后者会把慢任务退化成永不停歇的忙碌循环（ZT81 实测：
/// hr_enforce 38.6s/轮 + 60s 周期 → 实际周期缩到 45~58s，worker 全程扫表）。
fn every(secs: u64) -> tokio::time::Interval {
    let mut i = tokio::time::interval(std::time::Duration::from_secs(secs));
    i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    i
}

/// 银行结算日切门（原 run.rs 60s 分支内联块抽出）：
/// 站点时区 UTC+8 自然日切换后跑一次；分钟级检查保证重启/宕机跨日也能补跑。
async fn bank_daily_gate(
    db: &PgPool,
    last_bank_day: &mut Option<chrono::NaiveDate>,
) {
    let mut state = *last_bank_day;
    let site_day =
        (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    if state.is_none() {
        state = Some(init_bank_day(db).await);
    }
    if state != Some(site_day) {
        tracing::info!(?site_day, "bank_daily start");
        // bank_daily 内部各子步骤自带日期游标/幂等键，锁内重跑安全
        let db2 = db.clone();
        with_lock(db, "job:bank_daily", async move {
            crate::bank_jobs::bank_daily(&db2).await;
            Ok::<(), anyhow::Error>(())
        })
        .await;
        let after: Option<chrono::NaiveDate> =
            sqlx::query_scalar("SELECT max(run_date) FROM bank_settle_runs")
                .fetch_one(db)
                .await
                .ok()
                .flatten();
        state = Some(after.unwrap_or(site_day - chrono::Duration::days(1)));
    }
    *last_bank_day = state;
}

pub async fn run_all(
    db: PgPool,
    redis: redis::aio::ConnectionManager,
) -> anyhow::Result<()> {
    let mut tick = every(60);
    // 重型巡检 O(全表) 300s；0071 反作弊/性能分档
    let mut tick5 = every(300);
    let mut hour_tick = every(3600);
    let mut last_bank_day: Option<chrono::NaiveDate> = None;
    let mut tick10 = every(600);
    let mut tick30 = every(1800);
    let mut tick6h = every(6 * 3600);
    let mut tick1d = every(24 * 3600);
    let mut first_tick10 = true;
    let mut first_tick30 = true;
    let mut first_tick6h = true;
    let mut first_tick1d = true;
    // 手动任务句柄收口（0224 G30）：停机时 drain 等待在跑任务，而非裸 spawn 弃置
    let mut manual_tasks = crate::shutdown::TaskSet::new();
    let shard = job_shard();
    if let Some(list) = &shard {
        tracing::info!(jobs = list.join(","), "角色分片生效");
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
            // 多实例互斥（with_lock：advisory lock + 900s 超时，拿不到锁静默
            // 跳过本轮；失败/超时在 with_lock 内统一记日志）
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
            // 交叉上报消费（2026-10-07 P0-2 治本）：把 leecher 佐证的上传量
            // 累加进 upload_corroborated，作为计费侧上传量的可信上界。
            {
                let (db2, mut r) = (db.clone(), redis.clone());
                crate::jobs::locks::mark_start(&db,
                    "consume_xreport").await;
                if let Err(e) = consume_xreport(&db2, &mut r).await {
                    tracing::error!(?e, "consume_xreport 失败");
                }
                crate::jobs::locks::mark_end(&db, "consume_xreport",
                    None).await;
            }
            // ZT81：分离到后台执行，不再 join/await——原实现会把整个 select 循环
            // 卡到任务跑完，全表任务一慢，同分支的计费消费就跟着停摆。
            {
                let (db2, shard2) = (db.clone(), shard.clone());
                tokio::spawn(async move {
                    let mut js = tokio::task::JoinSet::new();
                    spawn_lock!(js, &db2, "job:backfill_pieces_hash",
                                backfill_pieces_hash, &shard2);
                    spawn_lock!(js, &db2, "job:sweep_stale_peers",
                                sweep_stale_peers, &shard2);
                    spawn_lock!(js, &db2, "job:task_settle",
                                crate::task_jobs::task_settle, &shard2);
                    spawn_lock!(js, &db2, "job:exam_assign",
                                crate::task_jobs::exam_assign, &shard2);
                    spawn_lock!(js, &db2, "job:lottery_settle",
                                lottery_settle_due, &shard2);
                    while let Some(res) = js.join_next().await {
                        let _ = res.map_err(|e| tracing::error!(?e, "join 失败"));
                    }
                });
            }
            bank_daily_gate(&db, &mut last_bank_day).await;
        }
        _ = hour_tick.tick() => {
            // 首轮不跳过（幂等键护栏，重跑零副作用）；僵尸阈值同 tick 现算
            let stale_secs = stale_peer_threshold_secs(&db).await;
            shard_lock!(&db, "job:seeding_reward",
                seeding_reward(&db, stale_secs), &shard);
            shard_lock!(&db, "job:purge_old_login_events",
                purge_old_login_events(&db), &shard);
            shard_lock!(&db, "job:ratio_watch", ratio_watch(&db), &shard);
            shard_lock!(&db, "job:dormant_mark", dormant_mark(&db), &shard);
            shard_lock!(&db, "job:request_expire",
                request_expire(&db), &shard);
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
            // 绩效月末结算挂 hourly（0106）：daily 首轮被跳过、重启后要等 24h
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
            // 每日做种满 6h 发口粮券（幂等靠发放表 PK）
            shard_lock!(&db, "job:game_coupons",
                grant_food_coupons(&db), &shard);
            // 字幕三扫（0148/0149）+ 卫生清理簇（邀请回收/凭证/日志/勋章）
            shard_lock!(&db, "job:subreq_sweep", subreq_sweep(&db), &shard);
            shard_lock!(&db, "job:subawards", subawards_build(&db), &shard);
            shard_lock!(&db, "job:subcert_sweep", subcert_sweep(&db), &shard);
            shard_lock!(&db, "job:expire_medals", expire_medals(&db), &shard);
            shard_lock!(&db, "job:expire_invites",
                expire_invites(&db), &shard);
            shard_lock!(&db, "job:purge_expired_tokens",
                purge_expired_tokens(&db), &shard);
            shard_lock!(&db, "job:purge_runtime_logs",
                purge_runtime_logs(&db), &shard);
            // DLQ 积压告警（只进不出等于变相丢计费）
            {
                let (db2, mut r) = (db.clone(), redis.clone());
                with_lock(&db, "job:dlq_watch", async move {
                    dlq_watch(&db2, &mut r).await
                })
                .await;
            }
        }
        _ = tick5.tick() => {
            // 重型巡检（ZT81 从 60s 迁来）：O(全表)，300s 一轮且分离到后台
            let (db2, shard2) = (db.clone(), shard.clone());
            tokio::spawn(async move {
                let mut js = tokio::task::JoinSet::new();
                spawn_lock!(js, &db2, "job:hr_enforce", hr_enforce, &shard2);
                spawn_lock!(js, &db2, "job:hr_punish", hr_punish, &shard2);
                spawn_lock!(js, &db2, "job:collect_milestones",
                            collect_milestones, &shard2);
                spawn_lock!(js, &db2, "job:class_auto_adjust",
                            class_auto_adjust, &shard2);
                spawn_lock!(js, &db2, "job:preserve_seed",
                            preserve_seed, &shard2);
                while let Some(res) = js.join_next().await {
                    let _ = res.map_err(|e| tracing::error!(?e, "j"));
                }
            });
        }
        _ = tick10.tick() => {
            if first_tick10 { first_tick10 = false; continue; }
            // 六轮 P1-C：检测任务不得与计费消费共抢调度（见 spawn_heavy）
            spawn_heavy(&db, &shard, |js, db2, shard2| {
                spawn_lock!(js, db2, "job:cheat_audit", cheat_audit, shard2);
                spawn_lock!(js, db2, "job:cheat_enforce",
                            cheat_enforce, shard2);
                spawn_lock!(js, db2, "job:collusion_check",
                            collusion_check, shard2);
            });
        }
        _ = tick30.tick() => {
            if first_tick30 { first_tick30 = false; continue; }
            // 同 P1-C：全表扫描后台化
            spawn_heavy(&db, &shard, |js, db2, shard2| {
                spawn_lock!(js, db2, "job:multi_ip_check",
                            multi_ip_check, shard2);
                spawn_lock!(js, db2, "job:leak_scan", leak_scan, shard2);
            });
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
            // 匿名统计（0276）：体内自查开关，未开即返回
            shard_lock!(&db, "job:usage_stats",
                usage_stats_report(&db), &shard); }        }
    }
    manual_tasks.drain().await;
    tracing::info!("flux-worker 已排空，退出");
    Ok(())
}

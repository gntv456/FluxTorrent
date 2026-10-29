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
    F: FnOnce(&mut tokio::task::JoinSet<()>, &PgPool, &Option<Vec<String>>)
        + Send
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

/// 分片内才执行一次 job（不需要 `continue` 的版本，供 spawn 出的任务体使用）。
async fn run_sharded<F, T>(
    db: &PgPool,
    shard: &Option<Vec<String>>,
    key: &'static str,
    fut: F,
) where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    if job_in_shard(key.trim_start_matches("job:"), shard) {
        with_lock(db, key, fut).await;
    }
}

/// 小时批：整体 spawn 出去，按原顺序逐条执行。
/// 原来直接在 select 分支里 await，medal_backfill 一次要按「勋章×用户」
/// 逐行回填（实测可跑数分钟），期间 60s 分支整个停摆——账单消费、
/// 僵尸 peer 清扫、手动触发认领一起停。这正是文件头 P1-C 要防的形状。
async fn hour_jobs(
    db: PgPool,
    mut redis: redis::aio::ConnectionManager,
    shard: Option<Vec<String>>,
) {
    // 僵尸阈值同 tick 现算（seeding_reward 也要用）
    let stale_secs = stale_peer_threshold_secs(&db).await;
    run_sharded(
        &db,
        &shard,
        "job:seeding_reward",
        seeding_reward(&db, stale_secs),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:purge_old_login_events",
        purge_old_login_events(&db),
    )
    .await;
    run_sharded(&db, &shard, "job:ratio_watch", ratio_watch(&db)).await;
    run_sharded(&db, &shard, "job:dormant_mark", dormant_mark(&db)).await;
    run_sharded(&db, &shard, "job:request_expire", request_expire(&db)).await;
    run_sharded(&db, &shard, "job:wishlist_notify", wishlist_notify(&db)).await;
    run_sharded(&db, &shard, "job:highspeed_tag", highspeed_tag(&db)).await;
    run_sharded(
        &db,
        &shard,
        "job:resurrection_settle",
        resurrection_settle(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:social_team_settle",
        social_team_settle(&db),
    )
    .await;
    // 失败判定必须排在成功结算之后：恰好在期限内达标的队伍应算成功
    run_sharded(
        &db,
        &shard,
        "job:social_team_expire",
        social_team_expire(&db),
    )
    .await;
    // 0382 勋章自动成就：threshold 周期重算 + 回填推进。
    // 两条都幂等可重跑（已授予即跳过、回填靠「扫到哪」续跑），
    // 故挂 hourly 而非 daily——threshold 的周期口径含日/周，
    // 一小时一轮才不会出现「周期内最后一天才达标的要等 24h」。
    // 0382 做种事件桥：把「本小时新增做种时长」记成可计数事件。
    // 放在 hourly 而非 announce 热路径——announce 每秒数千次，
    // 在那里插 INSERT 是拿全站最热的写表换一个低频事件源。
    run_sharded(
        &db,
        &shard,
        "job:medal_seeding_bridge",
        crate::jobs::medal_achv::record_seeding_hours(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:medal_threshold",
        crate::jobs::medal_achv::threshold_sweep(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:medal_backfill",
        crate::jobs::medal_achv::medal_backfill(&db),
    )
    .await;
    // 0383 商城履约自愈：把卡在 frozen 且过了宽限的订单推进到终态
    // 「钱不会被无限期占着」这条不变量的
    // 唯一保证——没有它，三段式只是把「扣款后崩掉」换成了
    // 「冻结后崩掉」。
    run_sharded(
        &db,
        &shard,
        "job:shop_fulfill_recover",
        crate::jobs::shop_fulfill::recover(&db),
    )
    .await;
    // 绩效月末结算挂 hourly（0106）：daily 首轮被跳过、重启后要等 24h
    run_sharded(&db, &shard, "job:jixiao_settle", jixiao_settle(&db)).await;
    run_sharded(&db, &shard, "job:preserve_settle", preserve_settle(&db)).await;
    run_sharded(&db, &shard, "job:funding_settle", funding_settle(&db)).await;
    run_sharded(&db, &shard, "job:refundable_settle", refundable_settle(&db))
        .await;
    run_sharded(&db, &shard, "job:achievement_grant", achievement_grant(&db))
        .await;
    // 每日做种满 6h 发口粮券（幂等靠发放表 PK）
    run_sharded(&db, &shard, "job:game_coupons", grant_food_coupons(&db)).await;
    // 字幕三扫（0148/0149）+ 卫生清理簇（邀请回收/凭证/日志/勋章）
    run_sharded(&db, &shard, "job:subreq_sweep", subreq_sweep(&db)).await;
    run_sharded(&db, &shard, "job:subawards", subawards_build(&db)).await;
    run_sharded(&db, &shard, "job:subcert_sweep", subcert_sweep(&db)).await;
    run_sharded(&db, &shard, "job:expire_medals", expire_medals(&db)).await;
    run_sharded(&db, &shard, "job:expire_invites", expire_invites(&db)).await;
    run_sharded(
        &db,
        &shard,
        "job:purge_expired_tokens",
        purge_expired_tokens(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:purge_runtime_logs",
        purge_runtime_logs(&db),
    )
    .await;
    // 审计日志归档（0357）：小时内检查，Redis 键节流每日最多一批
    run_sharded(
        &db,
        &shard,
        "job:audit_retain",
        audit_retain_tick(&db, &redis),
    )
    .await;
    // DLQ 积压告警（只进不出等于变相丢计费）
    {
        let (db2, mut r) = (db.clone(), redis.clone());
        run_sharded(&db, &shard, "job:dlq_watch", async move {
            dlq_watch(&db2, &mut r).await
        })
        .await;
    }
}

/// 消费者组任务统一入口（0225 G30-B2）：组内投递唯一，多实例并发消费是
/// 真分摊而非双计——不走 advisory 锁；mark_start/end 保留登记，面板
/// executed_by 可见分实例。错误只记不中断（下一轮自然补消费）。
async fn run_consumer<Fut>(db: &PgPool, name: &str, fut: Fut)
where
    Fut: std::future::Future<Output = anyhow::Result<u64>>,
{
    crate::jobs::locks::mark_start(db, name).await;
    if let Err(e) = fut.await {
        tracing::error!(?e, name, "消费失败");
    }
    crate::jobs::locks::mark_end(db, name, None).await;
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
async fn bank_daily_gate(db: &PgPool, last: &mut Option<chrono::NaiveDate>) {
    let mut state = *last;
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
    *last = state;
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
            // 跳过本轮；失败/超时记日志）
            shard_lock!(&db, "job:expire_promotions",
                expire_promotions(&db), &shard);
            // 邮件 outbox 消费（0404 §17-2）
            shard_lock!(&db, "job:mail_outbox",
                mail_out::mail_outbox_drain(&db), &shard);
            shard_lock!(&db, "job:magic_pool_promo",
                magic_pool_promo(&db), &shard);
            shard_lock!(&db, "job:announce_site_promos", announce_site_promos(&db), &shard);
            shard_lock!(&db, "job:preserve_exit", preserve_exit(&db), &shard);
            // consume_* 已改消费者组（0225 G30-B2）：组内投递唯一，多实例
            // 并发消费是真分摊而非双计——不再需要 advisory 锁互斥（保留
            // mark_start 登记则由面板覆盖写口径承担，executed_by 可见分实例）。
            let (db2, mut r) = (db.clone(), redis.clone());
            run_consumer(&db, "consume_announce",
                consume_announce(&db2, &mut r)).await;
            let (db2, mut r) = (db.clone(), redis.clone());
            run_consumer(&db, "consume_agent_blocks",
                consume_agent_blocks(&db2, &mut r)).await;
            // 交叉上报消费（2026-10-07 P0-2 治本）：把 leecher 佐证的上传量
            // 累加进 upload_corroborated，作为计费侧上传量的可信上界。
            let (db2, mut r) = (db.clone(), redis.clone());
            run_consumer(&db, "consume_xreport",
                consume_xreport(&db2, &mut r)).await;
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
            // 首轮不跳过（幂等键护栏，重跑零副作用）。整批 spawn 出去，
            // 否则长任务（medal_backfill）会把 60s 分支一起卡住。
            let (db2, r2, shard2) = (db.clone(), redis.clone(), shard.clone());
            tokio::spawn(async move {
                hour_jobs(db2, r2, shard2).await;
            });
        }
        _ = tick5.tick() => {
            // 重型巡检（ZT81 从 60s 迁来）：O(全表)，300s 一轮且分离到后台
            let (db2, shard2) = (db.clone(), shard.clone());
            tokio::spawn(async move {
                let mut js = tokio::task::JoinSet::new();
                spawn_lock!(js, &db2, "job:hr_enforce", hr_enforce, &shard2);
                spawn_lock!(js, &db2, "job:hr_warn_ladder",
                            hr_warn_ladder::run, &shard2);
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
                // 0368 M2 风险评分增量轮（含 decay 漂移兜底，见 risk_score.rs）
                spawn_lock!(js, db2, "job:risk_score",
                            risk_score_tick, shard2);
                spawn_lock!(js, db2, "job:collusion_check",
                            collusion_check, shard2);
                // 自动审种引擎轮（0400）
                spawn_lock!(js, db2, "job:auto_review",
                            auto_review::auto_review_tick, shard2);
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
            let (db2, shard2) = (db.clone(), shard.clone());
            tokio::spawn(async move {
                // 对账告警必须先于 reconcile_snapshots：收敛会抹掉差异证据
                run_sharded(&db2, &shard2, "job:reconcile_diff_alert",
                            reconcile_diff_alert(&db2)).await;
                run_sharded(&db2, &shard2, "job:reconcile_snapshots",
                            reconcile_snapshots(&db2)).await;
            });
        }
        _ = tick1d.tick() => {
            if first_tick1d { first_tick1d = false; continue; }
            shard_lock!(&db, "job:ensure_partitions",
                ensure_partitions(&db), &shard);
            // 0368 M2 风险评分全量轮（权重调参安全网）
            shard_lock!(&db, "job:risk_score_daily",
                risk_score_daily(&db), &shard);
            shard_lock!(&db, "job:reg_cleanup", reg_cleanup(&db), &shard);
            // 匿名统计（0276）：体内自查开关
            shard_lock!(&db, "job:usage_stats",
                usage_stats_report(&db), &shard);
            // 自动审种每日摘要（0400 §4.3）
            shard_lock!(&db, "job:auto_review_digest",
                auto_review::auto_review_digest(&db), &shard);
            // 月度契约结算（0424 同类站对标 A2）：月初跑，结算上月契约。
            // 日频安全：只动 status='open' 且 month=上月的行，幂等。
            shard_lock!(&db, "job:contract_settle",
                contract_settle(&db), &shard);
            // 反作弊规则集回测（P-471）：catalog 一直写着 24h，可这里从来没有
            // 它的分支——只有面板手点才会跑。接上日频，版本取表里最高的两版。
            shard_lock!(&db, "job:risk_backtest",
                risk_backtest::backtest_latest(&db, 90), &shard); }        }
    }
    manual_tasks.drain().await;
    tracing::info!("flux-worker 已排空，退出");
    Ok(())
}

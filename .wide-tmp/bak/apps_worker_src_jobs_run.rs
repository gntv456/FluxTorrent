//! run_all 总调度 + with_lock + module_on + 计费乘数 + 单测。
//! 从 jobs.rs 按域拆出。

use super::*;
use sqlx::PgPool;

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
    loop {
        tokio::select! {
            _ = tick.tick() => {
                // 审计修复（多实例互斥 + 超时）：每个 job 包 advisory lock + 900s 超时。
                // 多 worker 部署时同 job 只有抢到锁的实例执行（拿不到锁静默跳过本轮）；
                // 卡死任务 15 分钟后被 timeout 掐掉、连接归还，不会拖垮整个调度循环。
                // 失败/超时在 with_lock 内统一记日志（含 key），此处无需再逐个 match。
                with_lock(&db, "job:expire_promotions", expire_promotions(&db)).await;
                with_lock(&db, "job:magic_pool_promo", magic_pool_promo(&db)).await;
                with_lock(&db, "job:preserve_exit", preserve_exit(&db)).await;
                // consume_* 依赖 Redis 游标，天然单游标推进；但多实例并发拉同一段流
                // 仍会双计——同样入锁。ConnectionManager 是 clone 句柄，clone 后移入闭包；
                // PgPool 同样 clone（Arc 池句柄，代价可忽略），避免与外层 &db 借用冲突。
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:consume_announce", async move {
                        consume_announce(&db2, &mut r).await
                    })
                    .await;
                }
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:consume_agent_blocks", async move {
                        consume_agent_blocks(&db2, &mut r).await
                    })
                    .await;
                }
                with_lock(&db, "job:backfill_pieces_hash", backfill_pieces_hash(&db)).await;
                with_lock(&db, "job:sweep_stale_peers", sweep_stale_peers(&db)).await;
                with_lock(&db, "job:collect_milestones", collect_milestones(&db)).await;
                with_lock(&db, "job:hr_enforce", hr_enforce(&db)).await;
                with_lock(&db, "job:hr_punish", hr_punish(&db)).await;
                with_lock(&db, "job:class_auto_adjust", class_auto_adjust(&db)).await;
                with_lock(&db, "job:preserve_seed", preserve_seed(&db)).await;
                with_lock(&db, "job:task_settle", crate::task_jobs::task_settle(&db)).await;
                with_lock(&db, "job:exam_assign", crate::task_jobs::exam_assign(&db)).await;
                // 论坛抽奖到点开奖（0126）：draw_at 已过且仍 open 的逐个开。
                // 开奖逻辑（CAS open→drawn + 按人幂等发放）在 sqlx 层面自守，这里独立
                // 实现一份轻量扫描（worker 不依赖 api crate），锁内重跑安全。
                {
                    let due: Vec<i64> = sqlx::query_scalar(
                        "SELECT topic_id FROM topic_lotteries WHERE status = 'open' AND draw_at <= now() LIMIT 50",
                    )
                    .fetch_all(&db)
                    .await
                    .unwrap_or_default();
                    for tid in due {
                        if let Err(e) = lottery_settle(&db, tid).await {
                            tracing::warn!(topic_id = tid, error = %e, "lottery_settle failed");
                        }
                    }
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
                        sqlx::query_scalar("SELECT max(run_date) FROM bank_settle_runs")
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
                with_lock(&db, "job:seeding_reward", seeding_reward(&db, stale_secs)).await;
                with_lock(&db, "job:purge_old_login_events", purge_old_login_events(&db)).await;
                with_lock(&db, "job:ratio_watch", ratio_watch(&db)).await;
                with_lock(&db, "job:dormant_mark", dormant_mark(&db)).await;
                with_lock(&db, "job:wishlist_notify", wishlist_notify(&db)).await;
                with_lock(&db, "job:highspeed_tag", highspeed_tag(&db)).await;
                with_lock(&db, "job:resurrection_settle", resurrection_settle(&db)).await;
                with_lock(&db, "job:social_team_settle", social_team_settle(&db)).await;
                // 失败判定必须排在成功结算之后：恰好在期限内达标的队伍应算成功
                with_lock(&db, "job:social_team_expire", social_team_expire(&db)).await;
                // 绩效考核月末结算（0106）：挂 hourly 而非 daily——daily tick 首轮被
                // first_tick1d 跳过、重启后要等 24h 才首跑；hourly 首轮立即执行，
                // 且本 job 幂等（settled_at 标记 + 发薪幂等键），空扫描是一次索引查询
                with_lock(&db, "job:jixiao_settle", jixiao_settle(&db)).await;
                with_lock(&db, "job:preserve_settle", preserve_settle(&db)).await;
                with_lock(&db, "job:funding_settle", funding_settle(&db)).await;
                with_lock(&db, "job:refundable_settle", refundable_settle(&db)).await;
                with_lock(&db, "job:achievement_grant", achievement_grant(&db)).await;
                // 卫生清理（NP docleanup 口径）：过期邀请落库回收 / 一次性凭证与重置 token 清理
                with_lock(&db, "job:expire_invites", expire_invites(&db)).await;
                with_lock(&db, "job:purge_expired_tokens", purge_expired_tokens(&db)).await;
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
                with_lock(&db, "job:cheat_audit", cheat_audit(&db)).await;
            }
            _ = tick30.tick() => {
                if first_tick30 { first_tick30 = false; continue; }
                with_lock(&db, "job:multi_ip_check", multi_ip_check(&db)).await;
                with_lock(&db, "job:leak_scan", leak_scan(&db)).await;
            }
            _ = tick6h.tick() => {
                if first_tick6h { first_tick6h = false; continue; }
                // 对账告警必须先于 reconcile_snapshots：收敛会抹掉差异证据
                with_lock(&db, "job:reconcile_diff_alert", reconcile_diff_alert(&db)).await;
                with_lock(&db, "job:reconcile_snapshots", reconcile_snapshots(&db)).await;
            }
            _ = tick1d.tick() => {
                if first_tick1d { first_tick1d = false; continue; }
                with_lock(&db, "job:ensure_partitions", ensure_partitions(&db)).await;
            }
        }
    }
}

/// 多实例互斥 + per-job 超时（审计修复）。
/// - 互斥：pg_try_advisory_lock(hashtext(key)) 拿不到（他实例在跑）→ 返回 None 静默跳过本轮；
/// - 超时：tokio::time::timeout 900s 掐掉卡死任务（超时按失败上报）；
/// - 连接口径：advisory lock 是会话级，lock/unlock 必须落在同一条连接上——
///   从 pool acquire 一条专用连接持锁，业务 future 用整个 pool（不占锁连接），
///   完成后在同一连接 unlock。业务超时被掐后 unlock 仍执行，锁不残留。
/// 论坛抽奖开奖（0126，worker 侧）：与 api 的 lottery_draw_core 同一套库表协议——
/// CAS open→drawn 防双开，中奖发放幂等键 `forum-lottery-win:{tid}:{uid}`（spark_ledger 自守），
/// 无人参与退回楼主（`forum-lottery-refund:{tid}`）。票费不分成（归入池的是楼主冻结的奖金，
/// 票费在本实现里是参与门槛而非奖池构成，避免开奖金额与冻结额错位）。
pub(crate) async fn lottery_settle(
    db: &PgPool,
    topic_id: i64,
) -> anyhow::Result<u64> {
    let meta: Option<(i32, i64, i64)> = sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint FROM topic_lotteries \
         WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .fetch_optional(db)
    .await?;
    let Some((winners, prize, _ticket)) = meta else {
        return Ok(0); // 已开/已取消：幂等静默
    };
    let n = sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Ok(0); // 并发对手（楼主手动开）赢了对局
    }
    let mut tx = db.begin().await?;
    // 中奖名单：数据库侧 random() 洗牌取前 N（抽签随机性不由应用层承担）
    let picked: Vec<i64> = sqlx::query_scalar(
        "UPDATE lottery_entries SET won = TRUE \
         WHERE topic_id = $1 AND user_id IN ( \
           SELECT user_id FROM lottery_entries WHERE topic_id = $1 ORDER BY random() LIMIT $2 \
         ) RETURNING user_id",
    )
    .bind(topic_id)
    .bind(winners)
    .fetch_all(&mut *tx)
    .await?;
    if picked.is_empty() {
        // 无人参与：奖金池整退楼主
        let op: i64 =
            sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
                .bind(topic_id)
                .fetch_one(&mut *tx)
                .await?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
            )
            .bind(format!("forum-lottery-refund:{topic_id}"))
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                let bal: i64 = sqlx::query_scalar(
                    "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 RETURNING spark_balance",
                )
                .bind(op)
                .bind(refund)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'forum_lottery_refund', $3, $4)",
                )
                .bind(op)
                .bind(refund)
                .bind(format!("forum-lottery-refund:{topic_id}"))
                .bind(bal)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        tracing::info!(
            topic_id,
            refund,
            "lottery settled: no entries, refunded"
        );
        return Ok(0);
    }
    // 发放（同事务逐人：幂等键存在则跳过，重跑安全）
    for uid in &picked {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .fetch_one(&mut *tx)
        .await?;
        if exists || prize <= 0 {
            continue;
        }
        let bal: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(prize)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'forum_lottery', $3, $4)",
        )
        .bind(uid)
        .bind(prize)
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .bind(bal)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(topic_id, winners = picked.len(), prize, "lottery settled");
    Ok(picked.len() as u64)
}

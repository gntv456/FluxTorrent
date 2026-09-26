//! 手动触发（0218 G7）：面板「执行」= 入队 job_triggers，worker 在 60s tick 认领。
//! 关键：run_named 分派到 **worker 本体**（与定时同一函数），并在 with_lock 下执行
//! （同一把 advisory 锁）——不再复刻 SQL，口径与定时永不漂移。

use super::*;
use sqlx::PgPool;

/// 单任务分派：返回给面板看的结果摘要（影响行数等）。
pub(crate) async fn run_named(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    name: &str,
) -> anyhow::Result<String> {
    Ok(match name {
        "expire_promotions" => n(expire_promotions(db).await?),
        "magic_pool_promo" => n(magic_pool_promo(db).await?),
        "preserve_exit" => n(preserve_exit(db).await?),
        "consume_announce" => n(consume_announce(db, redis).await?),
        "consume_agent_blocks" => n(consume_agent_blocks(db, redis).await?),
        "backfill_pieces_hash" => n(backfill_pieces_hash(db).await?),
        "sweep_stale_peers" => n(sweep_stale_peers(db).await?),
        "collect_milestones" => n(collect_milestones(db).await?),
        "hr_enforce" => d(hr_enforce(db).await?),
        "hr_punish" => d(hr_punish(db).await?),
        "class_auto_adjust" => d(class_auto_adjust(db).await?),
        "preserve_seed" => n(preserve_seed(db).await?),
        "task_settle" => n(crate::task_jobs::task_settle(db).await?),
        "exam_assign" => n(crate::task_jobs::exam_assign(db).await?),
        "lottery_settle" => n(lottery_settle_due(db).await?),
        "bank_daily" => {
            crate::bank_jobs::bank_daily(db).await;
            d(())
        }
        "seeding_reward" => {
            let secs = stale_peer_threshold_secs(db).await;
            n(seeding_reward(db, secs).await?)
        }
        "purge_old_login_events" => n(purge_old_login_events(db).await?),
        "ratio_watch" => n(ratio_watch(db).await?),
        "dormant_mark" => n(dormant_mark(db).await?),
        "wishlist_notify" => n(wishlist_notify(db).await?),
        "highspeed_tag" => n(highspeed_tag(db).await?),
        "resurrection_settle" => n(resurrection_settle(db).await?),
        "social_team_settle" => n(social_team_settle(db).await?),
        "social_team_expire" => n(social_team_expire(db).await?),
        "jixiao_settle" => n(jixiao_settle(db).await?),
        "preserve_settle" => n(preserve_settle(db).await?),
        "funding_settle" => n(funding_settle(db).await?),
        "refundable_settle" => n(refundable_settle(db).await?),
        "achievement_grant" => n(achievement_grant(db).await?),
        "subreq_sweep" => d(subreq_sweep(db).await?),
        "subawards" => d(subawards_build(db).await?),
        "subcert_sweep" => {
            let (off, on) = subcert_sweep(db).await?;
            format!("撤销 {off} / 授予 {on}")
        }
        "expire_invites" => n(expire_invites(db).await?),
        "purge_expired_tokens" => n(purge_expired_tokens(db).await?),
        "purge_runtime_logs" => n(purge_runtime_logs(db).await?),
        "dlq_watch" => n(dlq_watch(db, redis).await?),
        "cheat_audit" => n(cheat_audit(db).await?),
        "multi_ip_check" => n(multi_ip_check(db).await?),
        "leak_scan" => n(leak_scan(db).await?),
        "reconcile_diff_alert" => d(reconcile_diff_alert(db).await?),
        "reconcile_snapshots" => d(reconcile_snapshots(db).await?),
        "ensure_partitions" => d(ensure_partitions(db).await?),
        other => anyhow::bail!("未知任务 {other}"),
    })
}

fn n(v: u64) -> String {
    format!("影响 {v} 行")
}

fn d(_: ()) -> String {
    "完成".to_string()
}

/// 认领队列（60s tick 调用）：回收中断行 → 取最多 3 条待执行 → 各自 spawn 执行。
/// spawn 而非 await：单个长任务（银行结息等）不拖住整个调度循环。
pub(crate) async fn poll_manual_triggers(
    db: &PgPool,
    redis: &redis::aio::ConnectionManager,
) {
    // 中断回收：worker 崩溃/重启留下的 running 行 30 分钟后判失败（面板不转圈）
    let _ = sqlx::query(
        "UPDATE job_triggers SET status = 'failed', ok = false, \
         finished_at = now(), result = 'worker 中断（超时回收）' \
         WHERE status = 'running' \
         AND started_at < now() - interval '30 minutes'",
    )
    .execute(db)
    .await;
    let claimed: Vec<(i64, String)> = sqlx::query_as(
        "UPDATE job_triggers SET status = 'running', started_at = now() \
         WHERE id IN (SELECT id FROM job_triggers WHERE status = 'pending' \
         ORDER BY id LIMIT 3) RETURNING id, job",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for (id, job) in claimed {
        let (db2, mut r2) = (db.clone(), redis.clone());
        tokio::spawn(async move {
            let key = format!("job:{job}");
            let (status, ok, result) =
                match run_guarded(&db2, &mut r2, &key, &job).await {
                    Ok(note) => ("done", true, note),
                    Err(e) => ("failed", false, format!("{e:#}")),
                };
            let _ = sqlx::query(
                "UPDATE job_triggers SET status = $2, ok = $3, \
                 finished_at = now(), result = $4 WHERE id = $1",
            )
            .bind(id)
            .bind(status)
            .bind(ok)
            .bind(&result)
            .execute(&db2)
            .await;
        });
    }
}

/// 模块关/他实例在跑 → 明确失败原因（而不是像定时路径那样静默跳过）
async fn run_guarded(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    key: &str,
    job: &str,
) -> anyhow::Result<String> {
    if let Some(module) = job_module(key) {
        if !module_on(db, module).await {
            anyhow::bail!("模块已关闭（{module}），未执行");
        }
    }
    match with_lock(db, key, run_named(db, redis, job)).await {
        Some(note) => Ok(note),
        None => anyhow::bail!("未执行：另一实例正在跑同一任务（锁未抢到）"),
    }
}

//! 清扫类：过期 peer/登录事件/邀请/token/DLQ。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 僵尸 peer 判定阈值（秒）= max(2h, 2×announce_interval)。
/// 同时用于两处：① `sweep_stale_peers` 清理标记；② `seeding_reward` 结算前的数据新鲜度过滤
/// （结算是发钱动作，不能依赖"另一个 job 恰好跑过"）。
pub(crate) async fn stale_peer_threshold_secs(db: &PgPool) -> i64 {
    let interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    (interval * 2).max(7200)
}

/// 僵尸做种/下载标记清理：tracker peer 表 90s 超时即除名，但 DB 侧 snatches.seeding/leeching
/// 原本只在下一次 announce 时被覆盖 —— 客户端崩溃/卸载（无 stopped 事件）的行会永久保持
/// seeding=true，导致 seeding_reward 空转发钱（live 证据：27 行 last_seen 2 天前仍在领收益）
/// 与 torrents.seeders 虚高。阈值 = max(2h, 2×announce_interval)，远大于正常重汇报抖动。
pub(crate) async fn sweep_stale_peers(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_secs = stale_peer_threshold_secs(db).await;
    let res = sqlx::query(
        "UPDATE snatches SET seeding = FALSE, leeching = FALSE \
         WHERE (seeding OR leeching) AND last_seen_at < now() - ($1::bigint * interval '1 second')",
    )
    .bind(threshold_secs)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 登录事件留存清理：IP 属个人信息，90 天后删除（每小时一次，幂等）。
pub(crate) async fn purge_old_login_events(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM login_events WHERE created_at < \
     now() - interval '90 days'",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 过期邀请落库回收（NP docleanup 口径）：status=0 且过期的邀请码统一置 status=2。
/// 此前仅展示层 CASE 折算，库内 status 恒 0——按 status 统计的后台口径失真。
pub(crate) async fn expire_invites(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "UPDATE invites SET status = 2 WHERE status = 0 \
     AND expires_at <= now()",
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "expired invites recycled");
    }
    Ok(res.rows_affected())
}

/// 一次性凭证清理：download_keys（30 分钟）与 password_resets（30 分钟）过期即删。
/// password_resets 原本只在手动 POST /admin/docleanup 里清，无人点击则永久堆积。
pub(crate) async fn purge_expired_tokens(db: &PgPool) -> anyhow::Result<u64> {
    let a = sqlx::query("DELETE FROM download_keys WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    let b = sqlx::query("DELETE FROM password_resets WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    Ok(a + b)
}

/// 死信队列可见性（卫生 P1）：DLQ 只进不出等于变相丢计费。
/// 有积压时通知管理组信箱（复用 cheat_audit 告警模式），同一批积压只告警一次。
/// 0227 审查 P2：agent_block 死信分键（flux:agentblock:dlq），与计费死信
/// 分开统计、文案分源——两队列共用告警节奏。
pub(crate) async fn dlq_watch(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    let a_len: i64 = redis.llen("flux:announce:dlq").await.unwrap_or(0);
    let b_len: i64 = redis.llen("flux:agentblock:dlq").await.unwrap_or(0);
    let len: i64 = a_len + b_len;
    if len == 0 {
        // 队列清空后复位告警游标，下批积压可再次告警
        let _: () = redis.del("flux:announce:dlq:alerted").await.unwrap_or(());
        return Ok(0);
    }
    let alerted: i64 =
        redis.get("flux:announce:dlq:alerted").await.unwrap_or(0);
    if alerted == 0 {
        let body = format!(
            "死信队列当前积压 {len} 条（连续失败 6 次进入）：计费流 \
             flux:announce:dlq {a_len} 条（计费已跳过需人工补偿）、客户端拦截流 \
             flux:agentblock:dlq {b_len} 条（仅告警记录）。请按队列排查。"
        );
        let _: Result<_, _> = sqlx::query(
            "INSERT INTO staffmessages (user_id, subject, body, permission) \
             SELECT MIN(id), 'announce 死信队列积压告警', $1, 'cheater' FROM users WHERE class_id >= 90",
        )
        .bind(body)
        .execute(db)
        .await;
        let _: () = redis
            .set_ex("flux:announce:dlq:alerted", 1, 24 * 3600)
            .await
            .unwrap_or(());
        tracing::warn!(len, "announce DLQ backlog alerted");
    }
    Ok(len as u64)
}

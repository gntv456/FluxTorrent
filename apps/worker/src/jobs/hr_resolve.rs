//! H&R 违规「达标自清」（0286 从 hr.rs 拆出）。
//!
//! 拆出理由：违规通知与封禁通知都向会员承诺「持续做种达标后违规自动消除」，
//! 而旧实现里只有人工 Pardon 会写 `hr_violations.resolved_at` ⇒ 承诺是假的，
//! 达标的人仍背着旧违规往 3 次阈值累计、最终被停下载。
//! 本模块按 snatch 现值回判、消除违规并回签一条说明。

use sqlx::PgPool;

/// 违规数降回阈值后解除 H&R 自己下的下载锁。
///
/// 0286 关键约束：只解 `download_locked_by='hr'` 的行。旧实现不区分下锁方，
/// 版主为查作弊手动关掉的下载权限会在 5 分钟内被本作业悄悄放开——
/// 冻结形同无效（旧注释自己写着「不区分当初被禁原因」）。
pub(crate) async fn restore_hr_locks(
    db: &PgPool,
    limit: i64,
) -> anyhow::Result<u64> {
    let restored = sqlx::query(
        r#"
        UPDATE users u
        SET download_enabled = TRUE, download_locked_by = NULL
        WHERE u.status < 2
          AND NOT u.download_enabled
          AND u.download_locked_by = 'hr'
          AND COALESCE((SELECT count(*) FROM hr_violations v
                        WHERE v.user_id = u.id
                          AND v.resolved_at IS NULL), 0) < $1
          AND NOT EXISTS (
              SELECT 1 FROM users u2
              WHERE u2.id = u.id AND u2.ratio_watch_until IS NOT NULL
                AND u2.ratio_watch_until < now()
          )
        "#,
    )
    .bind(limit)
    .execute(db)
    .await?;
    let n = restored.rows_affected();
    if n > 0 {
        tracing::info!(n, limit, "H&R 违规降回阈值以下，已恢复下载权限");
    }
    Ok(n)
}

/// 返回本轮被消除的违规条数（0 时不发通知、不写日志）。
pub(crate) async fn resolve_met_violations(db: &PgPool) -> anyhow::Result<u64> {
    let cleared = sqlx::query(
        r#"
        WITH met AS (
            UPDATE hr_violations v SET resolved_at = now()
            FROM snatches s
            JOIN hr_snapshots h ON h.user_id = s.user_id
                               AND h.torrent_id = s.torrent_id
            WHERE v.user_id = s.user_id AND v.torrent_id = s.torrent_id
              AND v.resolved_at IS NULL
              AND s.seeded_seconds >= h.required_seconds
            RETURNING v.id, v.user_id, v.torrent_id
        ),
        mark AS (
            UPDATE hr_snapshots h SET status = 'satisfied', updated_at = now()
            FROM met m WHERE h.user_id = m.user_id
              AND h.torrent_id = m.torrent_id
              AND h.status = 'violated'
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, m.user_id, 'H&R 违规已消除',
               format('您此前一条 H&R 违规（种子 #%s）经复核已消除：该种子累计做种时长'
                      '已达到要求。您现在的未解决违规数已相应减少，无需再操作。'
                      '如下载权限曾被暂停，将在下一轮检查自动恢复。', m.torrent_id)
        FROM met m
        "#,
    )
    .execute(db)
    .await?;
    let n = cleared.rows_affected();
    if n > 0 {
        tracing::info!(n, "H&R 违规达标自清（含通知）");
    }
    Ok(n)
}

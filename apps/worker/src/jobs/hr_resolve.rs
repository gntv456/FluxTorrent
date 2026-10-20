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
          -- P-445：阶梯第 2 档起的「限下」有自己的 expires_at；本作业若按
          -- 「未解决违规 < 阈值」放人，会在一分钟内把阶梯的 14/30 天锁抹掉，
          -- 恢复承诺形同虚设。阶梯总开关关闭时整段不生效（与 warn_cfg 同口径）。
          AND NOT EXISTS (
              SELECT 1 FROM hr_warn_states w
              WHERE w.user_id = u.id
                AND w.step > 0
                AND w.restrict_download
                AND COALESCE((SELECT value FROM site_settings
                              WHERE name = 'hr_warn_enable'), '1')
                    NOT IN ('0', 'no')
          )
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

/// 返回本轮被消除的违规条数（0 时不写日志）。
///
/// P-440：这条「达标消除」信是站内三档 H&R 通知里唯一没有读者的一档——
/// 控制面板的「H&R 达标时通知我」`users.notify_hr`（0024）全站无消费点，
/// 关掉它的人照收。预警走 `notice_prefs.hr_prewarn`、违规确认走
/// `hr_violation`，达标这一档在 13 档偏好面板里根本没有对应键，
/// 所以按列判、不另立新键（新键属加档位，不是接线）。
/// 消除本身与发信分开计数：偏好关掉的是「打扰」，不是「违规已清」。
pub(crate) async fn resolve_met_violations(db: &PgPool) -> anyhow::Result<u64> {
    let row = sqlx::query_as::<_, (i64, i64)>(
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
        ),
        pm AS (
            INSERT INTO messages (sender_id, receiver_id, subject, body, kind)
            SELECT NULL, m.user_id, 'H&R 违规已消除',
                   format('您此前一条 H&R 违规（种子 #%s）经复核已消除：'
                          '该种子累计做种时长已达到要求。您现在的未解决违规数'
                          '已相应减少，无需再操作。如下载权限曾被暂停，将在'
                          '下一轮检查自动恢复；若处于警告限制期，则待警告'
                          '到期后恢复。', m.torrent_id),
                   'hr_fulfilled'
            FROM met m
            JOIN users u ON u.id = m.user_id
            WHERE u.notify_hr
            RETURNING 1
        )
        SELECT (SELECT count(*) FROM met), (SELECT count(*) FROM pm)
        "#,
    )
    .fetch_one(db)
    .await?;
    let (cleared, pm_sent) = row;
    if cleared > 0 {
        tracing::info!(
            cleared,
            pm_sent,
            "H&R 违规达标自清（发信数按各人达标通知开关，可小于清除数）"
        );
    }
    // count(*) 回 bigint，本函数对外口径一直是 u64（rows_affected 同形）
    Ok(cleared.max(0) as u64)
}

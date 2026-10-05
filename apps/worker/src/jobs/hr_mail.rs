//! H&R 预警邮件通道（0283 P1-8，从 hr.rs 拆出守 300 行门禁）。

use sqlx::PgPool;

/// 给「本轮刚预警」（prewarned_at 近 10 分钟）的用户逐个补邮件。
/// prewarned_at 在 hr_enforce 主 SQL 已置位，本函数天然幂等；
/// SMTP 装配与投递在 mail_out（尽力而为，失败只记日志）。
pub(crate) async fn prewarn_mails(db: &PgPool, prewarn_hours: i32) {
    let mails: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT u.email, format(
            '种子 #%s 距 H&R 截止不到 %s 小时（做种 %s/%s 小时），\
请恢复做种或用魔力免罪。详情见站内信。',
            h.torrent_id, $1::int,
            round(h.seeded_seconds / 3600.0, 1),
            round(h.required_seconds / 3600.0, 1))
        FROM hr_snapshots h
        JOIN users u ON u.id = h.user_id
        WHERE h.prewarned_at > now() - interval '10 minutes'
          AND COALESCE((u.notice_prefs->>'hr_prewarn')::boolean, true)
        LIMIT 200
        "#,
    )
    .bind(prewarn_hours)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for (email, body) in mails {
        super::mail_out::send_user_mail(
            db,
            Some(&email),
            "H&R 预警：请尽快补足做种",
            &body,
        )
        .await;
    }
}

/// 预警主 SQL（0283 从 hr.rs 拆出守门禁）：置位 prewarned_at + 站内信 +
/// push outbox 单事务 CTE；prewarn_hours ≤ 0 返回零行（关闭整段）。
pub(crate) async fn prewarn_run(
    db: &PgPool,
    prewarn_hours: i32,
) -> anyhow::Result<sqlx::postgres::PgQueryResult> {
    if prewarn_hours <= 0 {
        return Ok(sqlx::query("SELECT 1 WHERE false").execute(db).await?);
    }
    Ok(sqlx::query(
        r#"
        WITH due AS (
            UPDATE hr_snapshots SET prewarned_at = now(), updated_at = now()
            WHERE status = 'open' AND prewarned_at IS NULL
              AND seeded_seconds < required_seconds
              AND deadline < now() + make_interval(hours => $1::int)
            RETURNING user_id, torrent_id, seeded_seconds,
                      required_seconds, deadline
        ),
        pm AS (
            INSERT INTO messages (sender_id, receiver_id, subject, body)
            SELECT NULL, d.user_id,
                   'H&R 预警：请尽快补足做种',
                   format('你完成的种子 #%s 距 H&R 考察截止还剩不到 %s 小时'
                          '（截止 %s）。当前累计做种 %s 小时，需 %s 小时。'
                          '请尽快恢复做种；也可在「我的 H&R」页用魔力自助免罪。',
                          d.torrent_id, $2,
                          to_char(d.deadline AT TIME ZONE 'Asia/Shanghai',
                                  'YYYY-MM-DD HH24:MI'),
                          round(d.seeded_seconds / 3600.0, 1),
                          round(d.required_seconds / 3600.0, 1))
            FROM due d
            JOIN users u ON u.id = d.user_id
            WHERE COALESCE((u.notice_prefs->>'hr_prewarn')::boolean, true)
            RETURNING 1
        )
        -- Web Push outbox（0283 P0-1）：api 侧消费循环投递，偏好由其过滤
        INSERT INTO push_outbox (user_id, topic, title, body, dedupe_key)
        SELECT d.user_id, 'hr', 'H&R 预警：请尽快补足做种',
               format('种子 #%s 距 H&R 截止不到 %s 小时（做种 %s/%s 小时），请恢复做种。',
                      d.torrent_id, $2,
                      round(d.seeded_seconds / 3600.0, 1),
                      round(d.required_seconds / 3600.0, 1)),
               'hr_prewarn:' || d.torrent_id || ':' || d.user_id
        FROM due d
        JOIN users u ON u.id = d.user_id
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(prewarn_hours)
    .bind(prewarn_hours)
    .execute(db)
    .await?)
}

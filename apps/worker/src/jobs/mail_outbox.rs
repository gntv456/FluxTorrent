//! mail_outbox 消费轮（0404/0497）：从 mail_out.rs 拆出——行数门禁。
//! 认领-退避重试-死信告警-单用户频控（0497）都在本文件。

use super::mail_out::{
    contact_reply_to, send_via_smtp, site_scope, smtp_config,
};
use super::staff_alert::staff_alert_gated;
use sqlx::PgPool;

/// mail_outbox 消费轮（0404）：捞未发送行退避重试。幂等：sent_at
/// 置位即完成；attempts≥5 停试并落死信告警。run.rs 挂 60s 档。
pub(crate) async fn mail_outbox_drain(db: &PgPool) -> anyhow::Result<u64> {
    // 0497 单用户频控：24h 窗口内已投递 ≥ cap 的行本轮跳过（不计
    // 失败、不占重试次数）；cap=0 不限（滚动窗近似，与退避同源）。
    let cap = mail_user_cap(db).await;
    // 认领一批（FOR UPDATE SKIP LOCKED 防双 worker 重发；行内 attempts
    // 即重试计数，退避 = 60s * 2^attempts，由 WHERE 谓词表达）
    let rows: Vec<(i64, i64, String, String, String, Option<String>)> =
        sqlx::query_as(
            "SELECT o.id, o.user_id, o.to_email, o.subject, \
             o.body, o.html_part \
         FROM mail_outbox o \
         WHERE o.sent_at IS NULL AND o.attempts < 5 \
           AND (o.last_attempt_at IS NULL OR o.last_attempt_at < \
                now() - make_interval(secs => 60 * power(2, \
                     o.attempts)::int8)) \
           AND ($1 = 0 OR (SELECT count(*) FROM mail_outbox x \
                 WHERE x.user_id = o.user_id AND x.sent_at \
                   > now() - interval '24 hours') < $1) \
         ORDER BY o.id LIMIT 20 FOR UPDATE OF o SKIP LOCKED",
        )
        .bind(cap)
        .fetch_all(db)
        .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut sent = 0u64;
    let (site, _base) = site_scope(db).await;
    for (id, _uid, to, subject, body, html_part) in rows {
        let Some((url, from)) = smtp_config(db).await else {
            // SMTP 未配置：不算失败也不死信（配置问题，配置好了下一轮自然发）
            tracing::info!("mail_outbox: SMTP 未配置，待命");
            return Ok(sent);
        };
        // 三态（0461）：有 html=新行；FLUXMAIL2 前缀=过渡行；否则老行。
        let (text, html) = match &html_part {
            Some(h) if !h.is_empty() => (body, h.clone()),
            _ => {
                let (t, h) = flux_shared::mailtpl::open_envelope(&body);
                if !h.is_empty() {
                    let _ = sqlx::query(
                        "UPDATE mail_outbox \
                         SET body = $2, html_part = $3 \
                         WHERE id = $1 AND html_part IS NULL",
                    )
                    .bind(id)
                    .bind(&t)
                    .bind(&h)
                    .execute(db)
                    .await;
                }
                (t, h)
            }
        };
        let mail = flux_shared::mailtpl::Mail {
            subject,
            text,
            html,
            site: site.clone(),
        };
        let rt = contact_reply_to(db).await;
        let r = send_via_smtp(&url, &from, &to, &mail, rt.as_deref()).await;
        match r {
            Ok(()) => {
                let _ = sqlx::query(
                    "UPDATE mail_outbox SET sent_at = now(), attempts = \
                     attempts + 1 WHERE id = $1",
                )
                .bind(id)
                .execute(db)
                .await;
                sent += 1;
            }
            Err(e) => {
                let attempts: i32 = sqlx::query_scalar(
                    "UPDATE mail_outbox SET attempts = attempts + 1, \
                     last_error = $2, last_attempt_at = now() \
                     WHERE id = $1 RETURNING attempts",
                )
                .bind(id)
                .bind(e.to_string())
                .fetch_one(db)
                .await
                .unwrap_or(0);
                if attempts >= 5 {
                    // P-428：死信最该出声，收进唯一出口（组合件：
                    // 无在册职员 warn+0；11-17 收编 24h 主题窗闸防灌箱）。
                    let body = format!(
                        "outbox #{id} 发送 5 次失败：{e}（收件已脱敏）"
                    );
                    let went = staff_alert_gated(
                        db,
                        "staff_panel",
                        "邮件死信告警",
                        24,
                        &body,
                    )
                    .await;
                    if went {
                        tracing::warn!(id, to, "mail_outbox: 死信告警已发");
                    }
                }
            }
        }
    }
    Ok(sent)
}

/// 0497 单用户邮件日上限（mail_user_daily_cap，缺省 5，clamp 0-1000）。
async fn mail_user_cap(db: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'mail_user_daily_cap'), 5)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(5)
    .clamp(0, 1000)
}

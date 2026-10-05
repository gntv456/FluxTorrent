//! 批量发放后的 PM/邮件通知（0286 从 increment_bulk.rs 拆出守门禁）。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 单批通知：站内信必达；email=true 时查邮箱走 mailer 双通道（尽力而为）。
pub(super) async fn pm_chunk(
    db: &PgPool,
    chunk: &[i64],
    subject: &str,
    text: &str,
    sender_id: Option<i64>,
    email: bool,
) -> DomainResult<()> {
    if subject.trim().is_empty() || text.trim().is_empty() {
        return Ok(());
    }
    for uid in chunk {
        if email {
            // 双通道：站内信必达 + 邮件尽力（mailer 内部降级）
            let em: Option<String> = sqlx::query_scalar(
                "SELECT email FROM users WHERE id = $1",
            )
            .bind(uid)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
            crate::mailer::notify(db, *uid, em, subject.trim(), text.trim())
                .await;
        } else {
            let sql = "INSERT INTO messages \
                       (sender_id, receiver_id, subject, body) \
                       VALUES ($1, $2, $3, $4)";
            sqlx::query(sql)
                .bind(sender_id)
                .bind(uid)
                .bind(subject.trim())
                .bind(text.trim())
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    Ok(())
}

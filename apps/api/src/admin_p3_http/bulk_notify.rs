//! 批量发放后的 PM/邮件通知（0286 从 increment_bulk.rs 拆出守门禁）。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 单批通知：站内信 + 可选邮件双通道，两条通道同走 `system` 档偏好。
///
/// 口径（P-322 站方裁）：管理群发属**可关**的公告类——用户在面板关掉
/// 「系统通知」后不再收。但**必达类不走这条路**：封禁/停权、账号安全、
/// 管理裁决结果必须送达，走 `mailer::notify()`（不过偏好），
/// 由 `mailer` 那侧「notify=必达 / notify_kind=可关」的分界保证。
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
            let em: Option<String> =
                sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
                    .bind(uid)
                    .fetch_optional(db)
                    .await
                    .ok()
                    .flatten();
            crate::mailer::notify_kind(
                db,
                *uid,
                "system",
                em,
                subject.trim(),
                text.trim(),
            )
            .await;
        } else {
            let sql = "INSERT INTO messages \
                       (sender_id, receiver_id, subject, body, kind) \
                       SELECT $1, $2, $3, $4, 'system' \
                       WHERE u_notice_enabled($2, 'system')";
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

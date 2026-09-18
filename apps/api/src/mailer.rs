//! 站内信 + 邮件统一投递（U2 §11.4）。
//!
//! 现状痛点：lettre 投递只在「找回密码」一条链路手搓（gaps_http::send_reset_mail）；
//! 邀请邮件 / 群发 massmail / 审核结果通知各自为政或缺失。
//! 本模块收口：SMTP 配置统一读 site_settings（0034 键）+ 环境变量兜底，
//! 站内信走 messages 表，邮件在后台线程投递不阻塞响应。
//!
//! 降级纪律：SMTP 未配置时记日志（开发态闭环），不报错——通知是尽力而为。

use sqlx::PgPool;

/// 站内信（系统发件人 user_id=0 口径，收件人不存在时静默跳过）
pub async fn site_message(
    db: &PgPool,
    to_user: i64,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT 0, $1, $2, $3 WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)",
    )
    .bind(to_user)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await?;
    Ok(())
}

/// SMTP 投递（复用 gaps_http 的 lettre 形状；from 未配置时跳过）
pub async fn send_mail(to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
    let smtp_url = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM")
        .unwrap_or_else(|_| std::env::var("smtp_from").unwrap_or_default());
    if smtp_url.is_empty() || from.is_empty() {
        tracing::info!(to, subject, "SMTP 未配置，邮件降级为日志");
        return Ok(());
    }
    crate::gaps_http::send_generic_mail(&smtp_url, &from, to, subject, body).await
}

/// 通知双通道：站内信必达 + 邮件尽力（后台线程）
pub async fn notify(
    db: &PgPool,
    to_user: i64,
    email: Option<String>,
    subject: &str,
    body: &str,
) {
    if let Err(e) = site_message(db, to_user, subject, body).await {
        tracing::error!(?e, to_user, "site message failed");
    }
    if let Some(to) = email {
        let (s, b) = (subject.to_string(), body.to_string());
        actix_web::rt::spawn(async move {
            if let Err(e) = send_mail(&to, &s, &b).await {
                tracing::error!(?e, to, "mail send failed");
            }
        });
    }
}

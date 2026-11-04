//! 通知分发（U2 §11.4，从 mailer.rs 按域拆出，300 行门禁）：
//! `notify_kind` 按用户 notice_prefs 开关过滤，`notify` 站内信 + 邮件
//! 双投（SMTP 未配置时降级为日志，不报错）。邮件正文统一由
//! `flux_shared::mailtpl` 渲染好的 `Mail` 传入——本文件不再拼正文。

use flux_shared::mailtpl::Mail;
use sqlx::PgPool;

use super::mailer::{site_message, smtp_config};

pub async fn notify_kind(
    db: &PgPool,
    to_user: i64,
    kind: &str,
    email: Option<String>,
    mail: &Mail,
) {
    let prefs: serde_json::Value =
        sqlx::query_scalar("SELECT notice_prefs FROM users WHERE id = $1")
            .bind(to_user)
            .fetch_one(db)
            .await
            .unwrap_or(serde_json::json!({}));
    let enabled = prefs.get(kind).and_then(|v| v.as_bool()).unwrap_or(true);
    if !enabled {
        return;
    }
    super::mailer_notify::notify(db, to_user, email, mail).await;
}

pub async fn notify(
    db: &PgPool,
    to_user: i64,
    email: Option<String>,
    mail: &Mail,
) {
    if let Err(e) = site_message(db, to_user, &mail.subject, &mail.text).await {
        tracing::error!(?e, to_user, "site message failed");
    }
    if let Some(to) = email {
        // 后台线程也要走「站点设定优先」：先把配置查出来再移动进任务
        let cfg = smtp_config(db).await;
        let mail = mail.clone();
        actix_web::rt::spawn(async move {
            let Some(cfg) = cfg else {
                tracing::info!(to, subject = %mail.subject, "SMTP 未配置，邮件降级为日志");
                return;
            };
            if let Err(e) = super::mailer::send::send(&cfg, &to, &mail).await {
                tracing::error!(?e, to, "mail send failed");
            }
        });
    }
}

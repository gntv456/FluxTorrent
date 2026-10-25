//! 通知分发（U2 §11.4，从 mailer.rs 按域拆出，300 行门禁）：
//! `notify_kind` 按用户 notice_prefs 开关过滤，`notify` 站内信 + 邮件
//! 双投（SMTP 未配置时降级为日志，不报错）。

use sqlx::PgPool;

use super::mailer::{site_message, smtp_config};

pub async fn notify_kind(
    db: &PgPool,
    to_user: i64,
    kind: &str,
    email: Option<String>,
    subject: &str,
    body: &str,
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
    super::mailer_notify::notify(db, to_user, email, subject, body).await;
}

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
        // 后台线程也要走「站点设定优先」：先把配置查出来再移动进任务
        let cfg = smtp_config(db).await;
        let (s, b) = (subject.to_string(), body.to_string());
        actix_web::rt::spawn(async move {
            let Some(cfg) = cfg else {
                tracing::info!(to, subject = %s, "SMTP 未配置，邮件降级为日志");
                return;
            };
            if let Err(e) = crate::gaps_http::send_generic_mail(
                &cfg.url, &cfg.from, &to, &s, &b,
            )
            .await
            {
                tracing::error!(?e, to, "mail send failed");
            }
        });
    }
}

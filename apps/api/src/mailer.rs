//! 站内信 + 邮件统一投递（U2 §11.4）。
//!
//! 本模块收口：站内信走 messages 表，邮件在后台线程投递不阻塞响应。
//! SMTP 配置统一「后台设定优先（site_settings 0034 键），环境变量兜底」
//! （0208 P0 修复：此前只读 SMTP_URL 环境变量，站点设定页的 smtp_* 三键
//! 保存后无任何代码读取——站长以为配好了，实际一封信都不发）。
//!
//! 降级纪律：SMTP 未配置时记日志（开发态闭环），不报错——通知是尽力而为。

use sqlx::PgPool;

/// 后台可配的 SMTP 形状（smtp_* 设定键装配；host/from 任一为空视为未配置）
#[derive(Clone, Debug)]
pub struct SmtpConfig {
    /// 形如 smtps://user:pass@host:port 的完整 URL（装配产物）
    pub url: String,
    pub from: String,
}

/// 读取 SMTP 配置：site_settings.smtp_host/smtp_port/smtp_from 优先；
/// 未配置时回退环境变量 SMTP_URL/SMTP_FROM（compose 注入口，兼容既有部署）。
/// 凭据/加密（0211）：accountname/accountpassword/encryption 三键接入装配——
/// encryption=ssl（缺省，465）/tls（587 STARTTLS，拼 smtp:// 由 lettre 升级）/none
///（内网中继 smtp:// 明文）；账密嵌入 URL userinfo，与 SMTP_URL 形状一致。
pub async fn smtp_config(db: &PgPool) -> Option<SmtpConfig> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
         WHERE name IN ('smtp_host','smtp_port','smtp_from', \
         'accountname','accountpassword','encryption')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get = |k: &str| {
        rows.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    // 后台三键齐（host + from）才算「后台已配置」；端口缺省 465（ssl 假定）
    if let (Some(host), Some(from)) = (get("smtp_host"), get("smtp_from")) {
        let encryption = get("encryption").unwrap_or_else(|| "ssl".into());
        let default_port = if encryption == "tls" { "587" } else { "465" };
        let port = get("smtp_port").unwrap_or_else(|| default_port.to_string());
        let scheme = match encryption.as_str() {
            "tls" | "none" => "smtp",
            _ => "smtps",
        };
        let (user, pass) = (get("accountname"), get("accountpassword"));
        let auth = match (user, pass) {
            // userinfo 百分号编码（@ : / 三个保留字即可覆盖绝大多数账密）
            (Some(u), Some(p)) => {
                let enc =
                    |s: &str| s.replace('@', "%40").replace(':', "%3A").replace('/', "%2F");
                format!("{}:{}@", enc(&u), enc(&p))
            }
            _ => String::new(),
        };
        let url = format!("{scheme}://{auth}{host}:{port}");
        return Some(SmtpConfig { url, from });
    }
    // 兜底：环境变量（smtp:// 形状自带账号密码；后台无凭据时保持 env 优先的凭据口）
    let url = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_default();
    if url.is_empty() || from.is_empty() {
        None
    } else {
        Some(SmtpConfig { url, from })
    }
}

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

/// SMTP 投递（配置装配见 smtp_config；未配置时降级日志）。
/// 持 db 句柄的调用方优先用 send_mail_with（读站点设定）。
pub async fn send_mail(
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let Some(cfg) = smtp_config_common().await else {
        tracing::info!(to, subject, "SMTP 未配置，邮件降级为日志");
        return Ok(());
    };
    crate::gaps_http::send_generic_mail(&cfg.url, &cfg.from, to, subject, body)
        .await
}

/// notify 等已有 db 句柄的调用方用（一次查询装配，站点设定优先）
pub async fn send_mail_with(
    db: &PgPool,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let Some(cfg) = smtp_config(db).await else {
        tracing::info!(to, subject, "SMTP 未配置，邮件降级为日志");
        return Ok(());
    };
    crate::gaps_http::send_generic_mail(&cfg.url, &cfg.from, to, subject, body)
        .await
}

/// 无 db 句柄场景（后台线程无池）：直接读 env 兜底。
/// 主链路（notify/reset/massmail/邀请）都持有 db，走 smtp_config；
/// 此口仅保留给确实拿不到池的边角调用。
async fn smtp_config_common() -> Option<SmtpConfig> {
    let url = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_default();
    if url.is_empty() || from.is_empty() {
        None
    } else {
        Some(SmtpConfig { url, from })
    }
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
        // 后台线程也要走「站点设定优先」：先把配置查出来再移动进任务
        let cfg = smtp_config(db).await;
        let (s, b) = (subject.to_string(), body.to_string());
        actix_web::rt::spawn(async move {
            let Some(cfg) = cfg else {
                tracing::info!(to, subject = %s, "SMTP 未配置，邮件降级为日志");
                return;
            };
            if let Err(e) =
                crate::gaps_http::send_generic_mail(&cfg.url, &cfg.from, &to, &s, &b)
                    .await
            {
                tracing::error!(?e, to, "mail send failed");
            }
        });
    }
}

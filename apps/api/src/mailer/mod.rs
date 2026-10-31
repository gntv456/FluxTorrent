//! 站内信 + 邮件统一投递（U2 §11.4）。
//!
//! 本模块收口：站内信走 messages 表，邮件在后台线程投递不阻塞响应。
//! SMTP 配置统一「后台设定优先（site_settings 0034 键），环境变量兜底」
//! （0208 P0 修复：此前只读 SMTP_URL 环境变量，站点设定页的 smtp_* 三键
//! 保存后无任何代码读取——站长以为配好了，实际一封信都不发）。
//!
//! 降级纪律：SMTP 未配置时记日志（开发态闭环），不报错——通知是尽力而为。

use sqlx::PgPool;

mod probe;
use probe::smtp_reachable;

/// 邮件里「点开就到站内页面」的站点基址（P-100 收口）。
///
/// 站内页面链接（找回密码 `/reset?token=`、邮箱激活 `/email-verify?token=`、
/// 邀请函）必须用**站点对外地址**，口径与 rss / 支付回调 / sitemap 一致：
/// `PUBLIC_SITE_URL` 优先。此前各条邮件链各读一个名字——reset 读
/// `PUBLIC_API_URL`（那是 API 域名，拼出来是死链）、邀请读
/// `PUBLIC_WEB_URL`——现网只配了 `PUBLIC_SITE_URL`，于是找回密码即使 SMTP
/// 完全可用也一律拒发（实测 500 + 信箱 0 封）。旧两个名字继续作为回落读，
/// 已按它们部署的站点行为不变。空串按「未配置」处理（不设兜底 flag）。
pub fn site_base() -> Option<String> {
    ["PUBLIC_SITE_URL", "PUBLIC_WEB_URL", "PUBLIC_API_URL"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

/// 邮件能否真正投递到用户手里（P-102/P-104 的前置判据）：站点基址可解析，
/// 且 SMTP 已配并能**连上**（NOOP 探测，含 TLS/认证）。开发态 FLUX_DEV=1
/// 且未配 SMTP 时保留日志闭环，方便本地自测。
/// 判据为假时，找回密码/邮箱激活/重发等「必须发信」的动作应回
/// `DomainError::MailUnconfigured`（503/4103），而不是 500、静默建号或
/// 假报 `sent:true`。只判「配置键非空」不够：SMTP 主机宕机/端口被封时
/// 配置仍在，旧实现照样放行——实测配了连不上的 SMTP，注册 200 建号、
/// 验证信发送失败，重发仍回 sent:true，用户被困在收不到信的激活页。
pub async fn mail_deliverable(db: &PgPool) -> bool {
    if site_base().is_none() {
        return false;
    }
    match smtp_config(db).await {
        Some(cfg) => smtp_reachable(&cfg.url).await,
        None => std::env::var("FLUX_DEV").unwrap_or_default() == "1",
    }
}

/// SMTP 可达性见 `probe.rs`（0416 起从本文件按行数门禁拆出）。

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
                let enc = |s: &str| {
                    s.replace('@', "%40")
                        .replace(':', "%3A")
                        .replace('/', "%2F")
                };
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

/// 站内信（系统发件人 user_id=0 口径，收件人不存在时静默跳过）。
/// 0417 F9：统一打 kind='system'——此前经此管线的通知（账号状态变更/
/// 申诉回执等）kind 恒 NULL，消息中心分类过滤无从落地；特定 kind 的
/// 通知走各域自带 kind 的直写 INSERT。
pub async fn site_message(
    db: &PgPool,
    to_user: i64,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO messages (receiver_id, subject, body, kind) \
         SELECT $1, $2, $3, 'system' \
         WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)",
    )
    .bind(to_user)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await?;
    Ok(())
}

/// 带文案键的站内信（0288 / P1-9）。
///
/// `kind` + `params` 让「先存下来、以后再展示」的文本可事后翻译：`subject/body`
/// 仍写按当前站点语言成文的兜底文本（老客户端、邮件、以及按键取不到译文时用），
/// 但前端在 `kind` 命中字典时优先现取字典渲染 ⇒ 切语言翻得动。
/// 项目 09-27 已定这条口径（抽卡流水那次是存了成品中文，切语言永远翻不过来）。
pub async fn site_message_keyed(
    db: &PgPool,
    to_user: i64,
    subject: &str,
    body: &str,
    kind: &str,
    params: serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO messages
        (receiver_id, subject, body, kind, params)
        SELECT $1, $2, $3, $4, $5::jsonb
        WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)"#,
    )
    .bind(to_user)
    .bind(subject)
    .bind(body)
    .bind(kind)
    .bind(params)
    .execute(db)
    .await?;
    Ok(())
}

/// 邮件风暴防护（0400 §4.2）：同一收件人 1 小时内同类邮件超 3 封时，
/// 后续只发站内信。键 = mailcap:{user}:{bucket}，Redis INCR+EXPIRE。
/// 只在「自动审种拒绝」类调用（人工审核/关键流程不走此闸）。
pub async fn mail_rate_allow(
    redis: &mut redis::aio::ConnectionManager,
    user_id: i64,
) -> bool {
    use redis::AsyncCommands;
    let key = format!(
        "mailcap:{user_id}:{}",
        chrono::Utc::now().timestamp() / 3600
    );
    let n: i64 = redis.incr(&key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = redis.expire(&key, 3700).await.unwrap_or(());
    }
    n <= 3
}

/// 邮件入 outbox（0404 §17-2）：自动审种拒信等「丢了用户就不知情」的
/// 邮件走这里——api 只入队，worker（job:mail_outbox）退避重试。
/// dedupe_key 幂等：同键重复入队被吞（重放安全）。
pub async fn enqueue_mail_outbox(
    db: &sqlx::PgPool,
    user_id: i64,
    to_email: &str,
    subject: &str,
    body: &str,
    dedupe_key: &str,
) {
    let r = sqlx::query(
        "INSERT INTO mail_outbox (user_id, to_email, subject, body, \
           dedupe_key) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (dedupe_key) DO NOTHING",
    )
    .bind(user_id)
    .bind(to_email)
    .bind(subject)
    .bind(body)
    .bind(dedupe_key)
    .execute(db)
    .await;
    if let Err(e) = r {
        tracing::warn!(?e, user_id, "mail_outbox 入队失败（退化为尽力直发）");
        let _ = send_mail_with(db, to_email, subject, body).await;
    }
}

/// notify 的带键版：站内信按键落库 + 邮件尽力（邮件正文只能是成品文本，保持原样）
pub async fn notify_keyed(
    db: &PgPool,
    to_user: i64,
    email: Option<String>,
    subject: &str,
    body: &str,
    kind: &str,
    params: serde_json::Value,
) {
    if let Err(e) =
        site_message_keyed(db, to_user, subject, body, kind, params).await
    {
        tracing::error!(?e, to_user, "site message failed");
    }
    if let Some(to) = email {
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

// 通知分发（notify_kind/notify）拆至 mailer_notify.rs（300 行门禁），
// 对外路径保持 crate::mailer::notify 不变（调用方无须改 import）
pub(crate) use super::mailer_notify::{notify, notify_kind};

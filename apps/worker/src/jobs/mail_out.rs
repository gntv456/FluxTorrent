//! Worker 侧邮件通道（0283 P1-8）：H&R 预警等批事件的邮件提醒。
//!
//! SMTP 配置口径与 api 侧 mailer::smtp_config 完全一致（site_settings 的
//! smtp_host/port/from + accountname/accountpassword/encryption 装配 URL），
//! 但 worker 无法引用 api crate（无 lib target），此处按同构 SQL 复读——
//! 两处装配逻辑如有变更须同步（与 push 的 outbox 解耦不同，邮件直接发）。

use sqlx::PgPool;

use super::staff_alert::staff_alert_gated;

/// 装配 SMTP 发信参数；未配置返回 None（调用方降级跳过）。
pub(crate) async fn smtp_config(db: &PgPool) -> Option<(String, String)> {
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
    let (Some(host), Some(from)) = (get("smtp_host"), get("smtp_from")) else {
        return None;
    };
    let encryption = get("encryption").unwrap_or_else(|| "ssl".into());
    let default_port = if encryption == "tls" { "587" } else { "465" };
    let port = get("smtp_port").unwrap_or_else(|| default_port.to_string());
    let scheme = match encryption.as_str() {
        "tls" | "none" => "smtp",
        _ => "smtps",
    };
    let auth = match (get("accountname"), get("accountpassword")) {
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
    Some((format!("{scheme}://{auth}{host}:{port}"), from))
}

/// 站点显示名与绝对基址（与 api `mailer::site_base` 同一组 env）。
pub(crate) async fn site_scope(db: &PgPool) -> (String, String) {
    let site: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let base = ["PUBLIC_SITE_URL", "PUBLIC_WEB_URL", "PUBLIC_API_URL"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();
    (
        site.map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "FluxTorrent".into()),
        base,
    )
}

/// percent 解码（与 api `decode_userinfo` 同口径）：装配 URL 时把账号里的
/// `@` 编成 `%40`，url crate 读回不解码，不还原就是 535 Login fail。
fn decode_userinfo(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let src = s.as_bytes();
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            if let (Some(h), Some(l)) = (hex(src[i + 1]), hex(src[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(src[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 给单用户发一封（errand 尽力而为）：email 为 None / SMTP 未配置 /
/// 投递失败都只记日志。
pub(crate) async fn send_user_mail(
    db: &PgPool,
    email: Option<&str>,
    mail: &flux_shared::mailtpl::Mail,
) {
    let Some(to) = email else { return };
    let Some((url, from)) = smtp_config(db).await else {
        return;
    };
    let rt = contact_reply_to(db).await;
    if let Err(e) = send_via_smtp(&url, &from, to, mail, rt.as_deref()).await {
        tracing::warn!(?e, to, subject = %mail.subject, "worker 邮件发送失败");
    }
}

/// 0461：Reply-To（contact_email 空/非法 = 不设；与 api send::reply_to
/// 同口径，worker 侧本地一份避免跨 crate 依赖 actix 栈）。
pub(crate) async fn contact_reply_to(db: &PgPool) -> Option<String> {
    let v = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'contact_email'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()?
    .trim()
    .to_string();
    if v.is_empty() || v.parse::<lettre::Address>().is_err() {
        tracing::warn!(value = %v, "contact_email 非法，不设 Reply-To");
        return None;
    }
    Some(v)
}

fn display_name(site: &str) -> Option<String> {
    let clean: String = site
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>'))
        .take(64)
        .collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

pub(crate) async fn send_via_smtp(
    smtp_url: &str,
    from: &str,
    to: &str,
    mail: &flux_shared::mailtpl::Mail,
    reply_to: Option<&str>,
) -> anyhow::Result<()> {
    use lettre::message::{Mailbox, MultiPart};
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let url = url::Url::parse(smtp_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("SMTP_URL 缺少主机"))?
        .to_string();
    let smtps = url.scheme() == "smtps";
    let port = url.port().unwrap_or(if smtps { 465 } else { 25 });
    let builder = if smtps {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
    }
    .port(port);
    use lettre::transport::smtp::authentication::Credentials;
    let builder = if !url.username().is_empty() {
        builder.credentials(Credentials::new(
            decode_userinfo(url.username()),
            decode_userinfo(url.password().unwrap_or_default()),
        ))
    } else {
        builder
    };
    let mailer = builder.build();
    let addr: lettre::Address = from.parse()?;
    let mut b = Message::builder()
        .from(Mailbox::new(display_name(&mail.site), addr))
        .to(to.parse()?)
        .subject(mail.subject.clone());
    if let Some(rt) = reply_to {
        b = b.reply_to(rt.parse()?);
    }
    // 历史 outbox 行只有纯文本；新行才有 HTML part。
    let msg = if mail.html.is_empty() {
        b.body(mail.text.clone())?
    } else {
        b.multipart(MultiPart::alternative_plain_html(
            mail.text.clone(),
            mail.html.clone(),
        ))?
    };
    mailer.send(msg).await?;
    Ok(())
}

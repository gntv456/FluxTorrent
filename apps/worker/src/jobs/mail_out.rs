//! Worker 侧邮件通道（0283 P1-8）：H&R 预警等批事件的邮件提醒。
//!
//! SMTP 配置口径与 api 侧 mailer::smtp_config 完全一致（site_settings 的
//! smtp_host/port/from + accountname/accountpassword/encryption 装配 URL），
//! 但 worker 无法引用 api crate（无 lib target），此处按同构 SQL 复读——
//! 两处装配逻辑如有变更须同步（与 push 的 outbox 解耦不同，邮件直接发）。

use sqlx::PgPool;

/// 装配 SMTP 发信参数；未配置返回 None（调用方降级跳过）。
async fn smtp_config(db: &PgPool) -> Option<(String, String)> {
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
    let (Some(host), Some(from)) = (get("smtp_host"), get("smtp_from"))
    else {
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
    Some((
        format!("{scheme}://{auth}{host}:{port}"),
        from,
    ))
}

/// 给单用户发邮件（尽力而为）：email 为 None / SMTP 未配置 / 投递失败都只记日志。
pub(crate) async fn send_user_mail(
    db: &PgPool,
    email: Option<&str>,
    subject: &str,
    body: &str,
) {
    let Some(to) = email else { return };
    let Some((url, from)) = smtp_config(db).await else {
        return;
    };
    if let Err(e) = send_via_smtp(&url, &from, to, subject, body).await {
        tracing::warn!(?e, to, subject, "worker 邮件发送失败");
    }
}

async fn send_via_smtp(
    smtp_url: &str,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let url = url::Url::parse(smtp_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("SMTP_URL 缺少主机"))?
        .to_string();
    let port =
        url.port().unwrap_or(if url.scheme() == "smtps" { 465 } else { 25 });
    let builder = if url.scheme() == "smtps" {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
    }
    .port(port);
    let builder = if !url.username().is_empty() {
        builder.credentials(
            lettre::transport::smtp::authentication::Credentials::new(
                url.username().to_string(),
                url.password().unwrap_or_default().to_string(),
            ),
        )
    } else {
        builder
    };
    let mailer = builder.build();
    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject.to_string())
        .body(body.to_string())?;
    mailer.send(email).await?;
    Ok(())
}

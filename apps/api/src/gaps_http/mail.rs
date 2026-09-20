//! 邮件通道与找回密码（0020）：SMTP 构建/发信/忘记与重置密码。
//! 从 gaps_http.rs 按域拆出；hash_token 在 login_aux.rs。

// ============ 找回密码（邮件通道） ============

/// 申请重置：生成 30 分钟有效 token。
/// 邮件投递：SMTP 未配置（SMTP_URL 空）时降级为日志输出 token —— 开发态可直接完成闭环；
/// 生产配 SMTP 后走真实投递（邮件发送在后台线程，不阻塞响应）。
/// SMTP 真实投递（lettre）：支持 smtps://user:pass@host:port 与 smtp://host:port 两种形状。
/// 连接失败/投递失败向上返回错误，由调用方记日志——忘记密码响应保持防枚举的统一文案。
pub async fn send_reset_mail(
    smtp_url: &str,
    from: &str,
    to: &str,
    site: &str,
    link: &str,
) -> anyhow::Result<()> {
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let url = url::Url::parse(smtp_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("SMTP_URL 缺少主机"))?
        .to_string();
    let port =
        url.port()
            .unwrap_or(if url.scheme() == "smtps" { 465 } else { 25 });
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
        .subject(format!("[{site}] 密码重置"))
        .body(format!(
            "你（或他人）在 {site} 申请了密码重置。

打开以下链接重置（30 分钟内有效，仅可使用一次）：
{link}

如非本人操作请忽略本邮件。"
        ))?;
    mailer.send(email).await?;
    Ok(())
}

/// 通用 SMTP 投递（U2 §11.4 mailer 收口）：主题+正文由调用方组装，
/// 连接构建逻辑与 send_reset_mail 同源。
pub async fn send_generic_mail(
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
        url.port()
            .unwrap_or(if url.scheme() == "smtps" { 465 } else { 25 });
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

/// 由 SMTP_URL 构建投递器（smtps://user:pass@host:port 或 smtp://host:port）。
/// 供忘记密码 / 邀请邮件等发送方共用。
pub fn build_smtp(
    smtp_url: &str,
) -> anyhow::Result<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>> {
    use lettre::{AsyncSmtpTransport, Tokio1Executor};

    let url = url::Url::parse(smtp_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("SMTP_URL 缺少主机"))?
        .to_string();
    let port =
        url.port()
            .unwrap_or(if url.scheme() == "smtps" { 465 } else { 25 });
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
    Ok(builder.build())
}

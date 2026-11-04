//! 邮件通道与找回密码（0020）：SMTP 构建/发信/忘记与重置密码。
//! 从 gaps_http.rs 按域拆出；hash_token 在 login_aux.rs。

/// userinfo 百分号解码（2026-11-04 535 实锤）：smtp_config 装配 URL 时把
/// 账号里的 `@` 编成 `%40`，而 `url::Url::username()/password()` 读回时
/// **不解码**——腾讯收到的用户名是 `97025804%40qq.com`，回 535 Login fail。
/// 现网对照实验：同码同主机，`%40` 形状稳定 535、`@` 形状 235。所有从
/// URL userinfo 取凭据的口子都必须过这一步解码。
fn decode_userinfo(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

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
                decode_userinfo(url.username()),
                decode_userinfo(url.password().unwrap_or_default()),
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
                decode_userinfo(url.username()),
                decode_userinfo(url.password().unwrap_or_default()),
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
                decode_userinfo(url.username()),
                decode_userinfo(url.password().unwrap_or_default()),
            ),
        )
    } else {
        builder
    };
    Ok(builder.build())
}

#[cfg(test)]
mod userinfo_tests {
    use super::decode_userinfo;

    /// 535 实锤（2026-11-04 现网）：smtp_config 把账号里的 `@` 编成 `%40`，
    /// url crate 读 userinfo 不解码 ⇒ 腾讯收到 `97025804%40qq.com` 稳定回
    /// 535 Login fail。解码后必须还原成完整邮箱地址。
    #[test]
    fn encoded_at_in_username_is_decoded() {
        assert_eq!(decode_userinfo("97025804%40qq.com"), "97025804@qq.com");
    }

    /// 装配层同样会编码密码里的保留字（: / @）。
    #[test]
    fn encoded_password_reserved_chars_are_decoded() {
        assert_eq!(decode_userinfo("p%3Aa%2Fb%40c"), "p:a/b@c");
    }

    /// 无编码的普通形状（env SMTP_URL 直填）不受影响。
    #[test]
    fn plain_userinfo_passes_through() {
        assert_eq!(decode_userinfo("plain"), "plain");
        assert_eq!(decode_userinfo(""), "");
    }
}

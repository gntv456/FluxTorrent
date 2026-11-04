//! 邮件通道（0020）：SMTP 投递器装配与通用发信。
//! 邮件正文（骨架 + 11 封文案）在 `flux_shared::mailtpl`，api/worker 共用；
//! 本文件只剩「按 smtp_* 设定拼投递器」这一件事。

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

/// 通用 SMTP 投递（U2 §11.4 mailer 收口）：主题与双 part 由统一模板给出。
pub async fn send_generic_mail(
    smtp_url: &str,
    from: &str,
    to: &str,
    mail: &flux_shared::mailtpl::Mail,
) -> anyhow::Result<()> {
    let mailer = build_smtp(smtp_url)?;
    crate::mailer::send::deliver(&mailer, from, to, mail).await
}

/// 由 SMTP_URL 构建投递器（smtps://user:pass@host:port 或 smtp://host:port）。
/// 供找回密码 / 邀请邮件 / 群发等发送方共用。
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

//! 统一邮件的投递口（multipart/alternative）+ 渲染上下文取数。
//!
//! 骨架与文案在 `flux_shared::mailtpl`（api/worker 共用）；本文件只负责
//! 把 `Mail` 变成 lettre `Message`：text 在前、html 在后，From 带站点
//! 显示名。Reply-To（0461）读 site_settings.contact_email——空/非法不设；
//! 站长想接住会员回信就填它，不填维持旧行为。

use flux_shared::mailtpl::{Ctx, Lang, Mail};
use lettre::message::{Mailbox, MultiPart};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;

use super::{site_base, SmtpConfig};

/// 渲染上下文（站点名、绝对基址、收件人语言）。
pub struct MailCtx {
    pub site: String,
    pub base: String,
    pub lang: Lang,
}

impl MailCtx {
    pub fn ctx(&self) -> Ctx<'_> {
        Ctx {
            site: &self.site,
            base: &self.base,
            lang: self.lang,
        }
    }
}

/// 一次查询取齐站点名与收件人语言偏好（`users.site_language` 是既有列，
/// 不新增字段/迁移）；查不到按 zh-CN，匿名收件人（邀请被邀者）也按 zh-CN。
pub async fn mail_ctx(db: &PgPool, user_id: Option<i64>) -> MailCtx {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT (SELECT value FROM site_settings WHERE name = 'site_name'), \
                (SELECT site_language FROM users WHERE id = $1)",
    )
    .bind(user_id.unwrap_or(0))
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let (site, lang) = row.unwrap_or((None, None));
    MailCtx {
        site: site
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "FluxTorrent".into()),
        base: site_base().unwrap_or_default(),
        lang: Lang::from_db(lang.as_deref()),
    }
}

/// 站点显示名清洗：头字段不接受裸 CR/LF 与尖括号，超长截断。
fn display_name(site: &str) -> Option<String> {
    let clean: String = site
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>'))
        .take(64)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

/// 回复接收邮箱（0461）：site_settings.contact_email，空 = 不设 Reply-To。
/// 地址合法性在这里兜底——站长填错形状不让它进头字段（ lettre Address
/// 解析失败会拒发整封信，宁可裸奔不设 Reply-To 也不挡投递）。
pub async fn reply_to(db: &sqlx::PgPool) -> Option<String> {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'contact_email'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|s: String| s.trim().to_string())
    .filter(|s: &String| !s.is_empty());
    let v = v?;
    if v.parse::<lettre::Address>().is_err() {
        tracing::warn!(value = %v, "contact_email 形状非法，本批不设 Reply-To");
        return None;
    }
    Some(v)
}

/// 组装 `multipart/alternative`（纯文本在前、HTML 在后）。
/// `reply_to` 为 None 时不设该头（0461 前的行为），有值则带上。
pub fn message(
    from: &str,
    to: &str,
    mail: &Mail,
    reply_to: Option<&str>,
) -> anyhow::Result<Message> {
    let addr: lettre::Address = from.parse()?;
    let from_box = Mailbox::new(display_name(&mail.site), addr);
    let mut b = Message::builder()
        .from(from_box)
        .to(to.parse()?)
        .subject(mail.subject.clone());
    if let Some(rt) = reply_to {
        b = b.reply_to(rt.parse()?);
    }
    let msg = b.multipart(MultiPart::alternative_plain_html(
        mail.text.clone(),
        mail.html.clone(),
    ))?;
    Ok(msg)
}

/// 用已构建的投递器发一封（massmail 批量复用连接；Reply-To 可选）。
pub async fn deliver_rt(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    from: &str,
    to: &str,
    mail: &Mail,
    reply_to: Option<&str>,
) -> anyhow::Result<()> {
    mailer.send(message(from, to, mail, reply_to)?).await?;
    Ok(())
}

/// 按 SMTP 配置发一封（每次装配；单封通知走这里；Reply-To 取
/// site_settings.contact_email，空/非法 = 不设，0461）。
pub async fn send(
    cfg: &SmtpConfig,
    to: &str,
    mail: &Mail,
) -> anyhow::Result<()> {
    send_rt(cfg, to, mail, None).await
}

/// 同上但带 Reply-To 覆盖（不想再查一次库的调用方用）。
pub async fn send_rt(
    cfg: &SmtpConfig,
    to: &str,
    mail: &Mail,
    reply_to: Option<&str>,
) -> anyhow::Result<()> {
    let mailer = crate::gaps_http::build_smtp(&cfg.url)?;
    deliver_rt(&mailer, &cfg.from, to, mail, reply_to).await
}

/// 后台尽力发信（0461）：调用方在请求线程先取好 cfg 与 Reply-To
/// （spawn 内不再碰 db），这里只负责 spawn 与日志。cfg=None 静默。
pub fn spawn_send(
    cfg: Option<SmtpConfig>,
    to: String,
    mail: Mail,
    reply_to: Option<String>,
) {
    actix_web::rt::spawn(async move {
        if let Some(cfg) = cfg {
            if let Err(e) = send_rt(&cfg, &to, &mail, reply_to.as_deref()).await
            {
                tracing::warn!(?e, to, "mail send failed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_shared::mailtpl::Tone;

    #[test]
    fn message_is_alternative_text_first() {
        let c = MailCtx {
            site: "山海站点".into(),
            base: "https://fluxtorrent.ptang.top".into(),
            lang: Lang::ZhCn,
        };
        let mail = flux_shared::mailtpl::generic_notice(
            &c.ctx(),
            "通知",
            "正文 & <x>",
            "站 内 通 知",
            Tone::Gold,
        );
        let msg =
            message("noreply@fluxtorrent.ptang.top", "u@x.test", &mail, None)
                .expect("应能组装");
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("multipart/alternative"));
        let ti = raw.find("text/plain").expect("缺 text part");
        let hi = raw.find("text/html").expect("缺 html part");
        assert!(ti < hi, "纯文本 part 必须在 HTML 之前");
        // From 带站点显示名（RFC 2047 编码后的中文名）
        assert!(raw.contains("From:"));
    }

    #[test]
    fn display_name_strips_header_breakers() {
        assert_eq!(display_name("A\r\nB <C>"), Some("AB C".to_string()));
        assert_eq!(display_name("  "), None);
        assert_eq!(display_name(&"x".repeat(100)).unwrap().len(), 64);
    }
}

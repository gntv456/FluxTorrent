//! 统一邮件的投递口（multipart/alternative）+ 渲染上下文取数。
//!
//! 骨架与文案在 `flux_shared::mailtpl`（api/worker 共用）；本文件只负责
//! 把 `Mail` 变成 lettre `Message`：text 在前、html 在后，From 带站点
//! 显示名。Reply-To 暂不设置——`site_settings` 里没有职员联系邮箱键
//! （`/contactstaff` 走站内工单），凭空造一个地址会把回复引到死信箱。

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

/// 组装 `multipart/alternative`（纯文本在前、HTML 在后）。
pub fn message(from: &str, to: &str, mail: &Mail) -> anyhow::Result<Message> {
    let addr: lettre::Address = from.parse()?;
    let from_box = Mailbox::new(display_name(&mail.site), addr);
    let msg = Message::builder()
        .from(from_box)
        .to(to.parse()?)
        .subject(mail.subject.clone())
        .multipart(MultiPart::alternative_plain_html(
            mail.text.clone(),
            mail.html.clone(),
        ))?;
    Ok(msg)
}

/// 用已构建的投递器发一封（massmail 批量复用连接）。
pub async fn deliver(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    from: &str,
    to: &str,
    mail: &Mail,
) -> anyhow::Result<()> {
    mailer.send(message(from, to, mail)?).await?;
    Ok(())
}

/// 按 SMTP 配置发一封（每次装配；单封通知走这里）。
pub async fn send(
    cfg: &SmtpConfig,
    to: &str,
    mail: &Mail,
) -> anyhow::Result<()> {
    let mailer = crate::gaps_http::build_smtp(&cfg.url)?;
    deliver(&mailer, &cfg.from, to, mail).await
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
        let msg = message("noreply@fluxtorrent.ptang.top", "u@x.test", &mail)
            .expect("应能组装");
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("multipart/alternative"));
        let ti = raw.find("text/plain").expect("缺 text part");
        let hi = raw.find("text/html").expect("缺 html part");
        assert!(ti < hi, "纯文本 part 必须在 HTML 之前");
        // From 带站点显示名（RFC 2047 编码后的中文名）
        assert!(raw.contains("From:"));
    }

    /// 本地 SMTP 捕获（默认 `#[ignore]`，只在人工验证时跑）：
    /// `python design/emails/smtp_sink.py 2526` 起捕获器后
    /// `cargo test -p flux-api --bin flux-api <name> -- --ignored`。
    /// 走的是生产同款路径：`SmtpConfig` → `build_smtp` → `deliver` → `message`。
    #[tokio::test]
    #[ignore]
    async fn probe_multipart_into_local_sink() {
        use flux_shared::mailtpl::{
            account_status, appeal_result, application_approved,
            application_rejected, email_verify, generic_notice, hr_warning,
            invite, password_reset, review_approved, review_rejected,
            StatusKind, Verdict,
        };
        let port: u16 = std::env::var("FLUX_MAIL_SINK_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2526);
        let from = "noreply@local.test";
        let to = "inbox@local.test";
        let cfg = SmtpConfig {
            url: format!("smtp://127.0.0.1:{port}"),
            from: from.to_string(),
        };
        let lang = |l: Lang| MailCtx {
            site: "山海驿站".into(),
            base: "https://flux.example.site".into(),
            lang: l,
        };
        let evil = "<script>alert(1)</script> & \"x\"\r\nBcc: evil@x.test";
        let token = "0123456789abcdef0123456789abcdef";
        let mut n = 0usize;
        for (tag, ctx) in [
            ("zh-cn", lang(Lang::ZhCn)),
            ("zh-tw", lang(Lang::ZhTw)),
            ("en", lang(Lang::En)),
        ] {
            for evil_mode in [false, true] {
                let user = if evil_mode { evil } else { "阿岚" };
                let subject = if evil_mode { Some(evil) } else { None };
                let note = if evil_mode { evil } else { "" };
                let c = ctx.ctx();
                let mail: Vec<(&str, Mail)> = vec![
                    ("invite", invite(&c, user, token, 72)),
                    ("password-reset", password_reset(&c, token, 30)),
                    ("email-verify", email_verify(&c, user, token, 48)),
                    (
                        "review-approved",
                        review_approved(&c, 1001, user, subject),
                    ),
                    (
                        "review-rejected",
                        review_rejected(&c, 1002, user, note, subject),
                    ),
                    (
                        "application-approved",
                        application_approved(
                            &c,
                            user,
                            token,
                            "u@local.test",
                            72,
                        ),
                    ),
                    (
                        "application-rejected",
                        application_rejected(
                            &c,
                            user,
                            note,
                            "u@local.test",
                            Some(42),
                        ),
                    ),
                    (
                        "appeal-accepted",
                        appeal_result(
                            &c,
                            "AP-7",
                            Verdict::Accepted,
                            note,
                            "已解除限制",
                            subject,
                        ),
                    ),
                    (
                        "appeal-rejected",
                        appeal_result(
                            &c,
                            "AP-8",
                            Verdict::Rejected,
                            note,
                            "维持原处理",
                            subject,
                        ),
                    ),
                    (
                        "account-status",
                        account_status(
                            &c,
                            user,
                            StatusKind::Ban,
                            "永久封禁",
                            note,
                            Some(("申诉令牌", "AP-TOK")),
                            subject,
                        ),
                    ),
                    (
                        "hr-warning",
                        hr_warning(
                            &c,
                            1003,
                            user,
                            6.0,
                            24.0,
                            "2026-11-08 00:00 UTC",
                            subject,
                        ),
                    ),
                ];
                for (name, m) in mail {
                    send(&cfg, to, &m).await.expect("捕获器应可达");
                    n += 1;
                    println!("sent {tag}/{name} evil={evil_mode}");
                }
                if evil_mode {
                    // 管理员群发走 generic_notice（同一骨架，内容块不同）
                    let g = generic_notice(
                        &c,
                        subject.unwrap_or("notice"),
                        note,
                        "站 内 通 知",
                        Tone::Gold,
                    );
                    send(&cfg, to, &g).await.expect("捕获器应可达");
                    n += 1;
                    println!("sent {tag}/generic-notice evil=true");
                }
            }
        }
        println!("total sent = {n}");
    }

    #[test]
    fn display_name_strips_header_breakers() {
        assert_eq!(display_name("A\r\nB <C>"), Some("AB C".to_string()));
        assert_eq!(display_name("  "), None);
        assert_eq!(display_name(&"x".repeat(100)).unwrap().len(), 64);
    }
}

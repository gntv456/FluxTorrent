//! 本地 SMTP 捕获探针（从 send.rs 拆出，行数门禁）：
//! `#[ignore]` 常规不跑；人工验证时
//! `python design/emails/smtp_sink.py 2526` 起捕获器后
//! `cargo test -p flux-api --bin flux-api probe_multipart -- --ignored`。

use super::send::{send, MailCtx};
use super::SmtpConfig;
use flux_shared::mailtpl::{Lang, Mail, Tone};

#[tokio::test]
#[ignore]
async fn probe_multipart_into_local_sink() {
    // 走生产同款路径：SmtpConfig → build_smtp → deliver_rt → message。
    use flux_shared::mailtpl::{
        account_status, appeal_result, application_approved,
        application_rejected, email_verify, generic_notice, hr_warning, invite,
        password_reset, review_approved, review_rejected, StatusKind, Verdict,
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
                ("review-approved", review_approved(&c, 1001, user, subject)),
                (
                    "review-rejected",
                    review_rejected(&c, 1002, user, note, subject),
                ),
                (
                    "application-approved",
                    application_approved(&c, user, token, "u@local.test", 72),
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

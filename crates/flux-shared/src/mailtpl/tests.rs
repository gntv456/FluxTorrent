//! 母版与 11 封内容函数的回归：转义、绝对链接、零外链资产、双 part。

use super::templates::*;
use super::*;

const SITE: &str = "山海测试站";
const BASE: &str = "https://fluxtorrent.ptang.top";
const EVIL: &str = "<script>alert(1)</script> & \"x\"";

fn ctx(lang: Lang) -> Ctx<'static> {
    Ctx {
        site: SITE,
        base: BASE,
        lang,
    }
}

fn all_mails() -> Vec<(&'static str, Mail)> {
    let c = ctx(Lang::ZhCn);
    let mut v: Vec<(&'static str, Mail)> = vec![
        ("invite", invite(&c, EVIL, "code_a&b<c>", 72)),
        ("password-reset", password_reset(&c, "pwd_&<x>", 30)),
        ("email-verify", email_verify(&c, EVIL, "tok_<&>", 48)),
        ("review-approved", review_approved(&c, 7001, EVIL, None)),
        (
            "review-rejected",
            review_rejected(&c, 7001, EVIL, EVIL, None),
        ),
        (
            "application-approved",
            application_approved(&c, EVIL, "code_<x>", "a&b@x.test", 72),
        ),
        (
            "application-rejected",
            application_rejected(&c, EVIL, EVIL, "a&b@x.test", Some(12)),
        ),
        (
            "appeal-accepted",
            appeal_result(
                &c,
                "888&<>",
                Verdict::Accepted,
                EVIL,
                "限制已解除",
                None,
            ),
        ),
        (
            "appeal-rejected",
            appeal_result(
                &c,
                "888&<>",
                Verdict::Rejected,
                EVIL,
                "维持原处理",
                None,
            ),
        ),
        (
            "account-status",
            account_status(
                &c,
                EVIL,
                StatusKind::Ban,
                "已封禁",
                EVIL,
                Some(("申诉凭证", "t<o>k")),
                None,
            ),
        ),
        (
            "hr-warning",
            hr_warning(&c, 7001, EVIL, 3.0, 72.0, "2026-11-06 12:00 UTC", None),
        ),
        (
            "generic",
            generic_notice(&c, "站点公告 <b>", EVIL, "站 内 通 知", Tone::Gold),
        ),
    ];
    v.sort_by(|a, b| a.0.cmp(b.0));
    v
}

#[test]
fn outbox_envelope_round_trips_and_keeps_legacy_rows() {
    let e = envelope("正文\n链接 https://x.test/a", "<p>正文</p>");
    let (t, h) = open_envelope(&e);
    assert_eq!(t, "正文\n链接 https://x.test/a");
    assert!(h.contains("<p>正文</p>"));
    // 历史纯文本行：没有前缀，原样当 text part
    let (t, h) = open_envelope("旧正文");
    assert_eq!(t, "旧正文");
    assert!(h.is_empty());
    // 前缀正确但 JSON 坏了：退回纯文本，不丢掉这封信
    let broken = format!("{}not-json", ENVELOPE_PREFIX);
    let (t, h) = open_envelope(&broken);
    assert_eq!(t, broken);
    assert!(h.is_empty());
}

#[test]
fn user_text_is_escaped_in_html_but_raw_in_text() {
    for (name, m) in all_mails() {
        assert!(
            !m.html.contains("<script>alert(1)</script>"),
            "{name}: HTML part 未转义用户输入"
        );
        if name != "password-reset" {
            assert!(
                m.html.contains("&lt;script&gt;"),
                "{name}: HTML part 缺少转义后的脚本片段"
            );
        }
        assert!(
            !m.html.contains("<b>") && !m.html.contains("<o>"),
            "{name}: HTML part 残留未转义标签"
        );
        // 纯文本部分保留原文（不转义），只在换行处保留语义
        if name != "invite"
            && name != "password-reset"
            && name != "email-verify"
        {
            assert!(
                m.text.contains("<script>alert(1)</script>"),
                "{name}: 纯文本 part 不应做 HTML 转义"
            );
        }
    }
}

#[test]
fn every_link_is_absolute_and_no_external_asset() {
    for (name, m) in all_mails() {
        for href in m.html.split("href=\"").skip(1) {
            let url = href.split('"').next().unwrap_or("");
            assert!(
                url.starts_with(BASE),
                "{name}: 链接不是绝对站内地址: {url}"
            );
        }
        assert!(!m.html.contains("<img"), "{name}: 不允许外部/内嵌图片");
        assert!(!m.html.contains("<script"), "{name}: 不允许脚本");
        assert!(!m.html.contains("@import"), "{name}: 不允许外部 CSS");
        assert!(!m.html.contains("url(http"), "{name}: 不允许远程背景图");
        // 纯文本 part 必须把按钮地址也写全
        if let Some(cta) = html_cta_url(&m.html) {
            let raw = cta
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'");
            assert!(m.text.contains(&raw), "{name}: 纯文本缺少完整链接 {raw}");
        }
    }
}

fn html_cta_url(html: &str) -> Option<String> {
    let i = html.find("mso-padding-alt")?;
    let rest = &html[i..];
    let j = rest.find("href=\"")?;
    let rest = &rest[j + 6..];
    Some(rest.split('"').next()?.to_string())
}

#[test]
fn skeleton_is_identical_across_mails() {
    let mails = all_mails();
    let first = &mails[0].1.html;
    for (name, m) in &mails[1..] {
        assert!(
            m.html.contains("fx-card") && m.html.contains("fx-band"),
            "{name}: 缺少统一骨架"
        );
        assert!(
            m.html.contains("background-color:#f7f3e8"),
            "{name}: 纸色底缺失"
        );
        // 装饰带高度只允许三档
        let band = m
            .html
            .split("height=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or("");
        assert!(
            matches!(band, "96" | "56" | "64"),
            "{name}: 装饰带档位异常 {band}"
        );
        assert!(
            first.contains("<style>") && m.html.contains("<style>"),
            "{name}: 头部样式块缺失"
        );
    }
}

#[test]
fn three_languages_are_rendered_with_matching_lang_attr() {
    for (lang, attr) in [
        (Lang::ZhCn, "lang=\"zh-CN\""),
        (Lang::ZhTw, "lang=\"zh-TW\""),
        (Lang::En, "lang=\"en\""),
    ] {
        let c = ctx(lang);
        for m in [
            invite(&c, "inviter", "abc", 72),
            password_reset(&c, "tok", 30),
            email_verify(&c, "user", "tok", 48),
            review_approved(&c, 1, "title", None),
            review_rejected(&c, 1, "title", "reason", None),
            application_approved(&c, "user", "code", "a@b.c", 72),
            application_rejected(&c, "user", "note", "a@b.c", None),
            appeal_result(&c, "9", Verdict::Accepted, "note", "ok", None),
            account_status(
                &c,
                "user",
                StatusKind::Restore,
                "restored",
                "ok",
                None,
                None,
            ),
            hr_warning(&c, 1, "title", 1.0, 72.0, "2026-11-06", None),
        ] {
            assert!(m.html.contains(attr), "{lang:?}: html lang 属性不一致");
            assert!(!m.text.starts_with("Subject:"), "正文不应带 Subject 行");
        }
    }
}

#[test]
fn subject_has_no_header_injection() {
    let c = ctx(Lang::En);
    let m = review_rejected(&c, 1, "t", "r", Some("Hi\r\nBcc: evil@x.test"));
    assert!(!m.subject.contains('\r') && !m.subject.contains('\n'));
    assert!(m.subject.contains("Hi"), "主题内容不应被整体丢弃");
}

#[test]
fn missing_base_degrades_to_no_relative_links() {
    let c = Ctx {
        site: SITE,
        base: "",
        lang: Lang::ZhCn,
    };
    let m = password_reset(&c, "tok", 30);
    assert_eq!(m.html.matches("href=\"").count(), 0);
    assert!(!m.html.contains("href=\"/"));
}

#[test]
fn long_text_wraps_but_urls_stay_intact() {
    let url = format!("{}/email-verify?token={}", BASE, "a".repeat(64));
    let content = Content {
        lang: Lang::ZhCn,
        site: SITE.into(),
        base: BASE.into(),
        tier: Tier::Hero,
        tone: Tone::Pending,
        seal: "验".into(),
        category: "邮 箱 激 活".into(),
        title: "请验证你的邮箱".into(),
        preheader: "验证后账号才能正常收信。".into(),
        intro: "这是一段很长的说明文字，".repeat(4),
        result: None,
        info: vec![],
        body: vec![],
        bullets: None,
        cta: Some(Cta {
            label: "验证邮箱".into(),
            url: url.clone(),
        }),
        cta_note: None,
        safety: None,
        footer: "页脚".into(),
    };
    let text = render_text(&content);
    assert!(text.contains(&url), "URL 被折行破坏");
    let long = text
        .lines()
        .find(|l| !l.starts_with("https://") && l.chars().count() > 72);
    assert!(long.is_none(), "普通段落应在 72 列内折行");
}

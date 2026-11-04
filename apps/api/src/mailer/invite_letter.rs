//! 邀请函邮件模板（2026-11-04）：multipart/alternative——HTML 品牌卡片
//! 为主、原纯文本为回落。收件人多半对站点毫无认知（是被朋友拉来的
//! 圈外人），这封信是站点的第一印象，值得一张「卡片」而不是 6 行裸文。
//!
//! 设计约束（邮件客户端现实）：
//! - QQ 邮箱/网易/Gmail 会剥离 <style> 与外链图片默认不显示 ⇒ 全部
//!   样式内联（style=""），logo 用 web 主域绝对地址并容忍不加载；
//! - 深色模式客户端会反转颜色 ⇒ 背景用浅色底 + 深字，不赌反色；
//! - 主按钮用大号 padding 的 <a>（table 按钮兼容性最好），链接同时
//!   以文本形式露出——「点不了按钮也能复制链接」是邀请函的底线。
//!
//! 站长个性化：站点名/logo/一句话简介全部取自 site_settings
//! （site_name / site_logo / metadescription），改后台即改邮件。

use lettre::message::MultiPart;

/// 一封邀请函的输入（调用方 invite_http/email.rs 一次查齐）。
pub(crate) struct InviteLetter {
    pub site: String,
    /// site_settings.site_logo（可空 = 不放图，放站名首字圆形徽标）
    pub logo: Option<String>,
    /// site_settings.metadescription（可空 = 用默认副标题）
    pub tagline: Option<String>,
    pub inviter: String,
    pub link: String,
    pub code: String,
    /// 剩余有效小时数（≥1）
    pub hours_left: i64,
}

/// HTML 转义（站名/用户名都是用户可控文本，进 HTML 前必须过）。
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 有效期人话：72h→「3 天」、30h→「30 小时」，比裸小时数好读。
fn human_ttl(hours: i64) -> String {
    if hours >= 48 && hours % 24 == 0 {
        format!("{} 天", hours / 24)
    } else {
        format!("{hours} 小时")
    }
}

/// 徽标格：有 logo 用图（容错 40×40），没有则站名首字圆形占位——
/// 两者的尺寸/圆角一致，收件人看到的版式不因缺图而塌。
fn logo_html(l: &InviteLetter) -> String {
    match &l.logo {
        Some(url) if !url.trim().is_empty() => format!(
            r#"<img src="{}" width="44" height="44" alt="{}" \
style="border-radius:12px;display:block;border:1px solid #e5e7eb">"#,
            esc(url.trim()),
            esc(&l.site)
        ),
        _ => {
            let ch = l.site.chars().next().unwrap_or('F');
            format!(
                r#"<div style="width:44px;height:44px;border-radius:12px;\
background:#1e293b;color:#f8fafc;font-size:22px;font-weight:700;\
display:flex;align-items:center;justify-content:center">{}</div>"#,
                esc(&ch.to_string())
            )
        }
    }
}

/// HTML 卡片主体。600px 是邮件安全宽度；外层留背景色，内层卡片
/// 白底圆角——多数客户端不支持 border-radius 也没关系，退化成直角。
fn html_body(l: &InviteLetter) -> String {
    let tagline = esc(l
        .tagline
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("一个热爱分享的资源社区"));
    let ttl = human_ttl(l.hours_left);
    format!(
        r#"<!doctype html>
<html><body style="margin:0;padding:0;background:#f1f5f9">
<div style="display:none;max-height:0;overflow:hidden">
{inviter} 邀请你加入 {site}——邀请码 {ttl}内有效</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" \
style="background:#f1f5f9;padding:32px 12px"><tr><td align="center">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" \
style="max-width:600px;width:100%">
<tr><td style="padding:0 8px 16px 8px">
  <table role="presentation" cellpadding="0" cellspacing="0"><tr>
    <td style="padding-right:12px">{logo}</td>
    <td style="font-family:-apple-system,'Segoe UI',Roboto,'PingFang SC',\
'Microsoft YaHei',sans-serif">
      <div style="font-size:20px;font-weight:700;color:#0f172a">{site}</div>
      <div style="font-size:13px;color:#64748b">{tagline}</div>
    </td>
  </tr></table>
</td></tr>
<tr><td style="padding:0 8px">
  <div style="background:#ffffff;border:1px solid #e2e8f0;border-radius:16px;\
padding:36px 32px;font-family:-apple-system,'Segoe UI',Roboto,'PingFang SC',\
'Microsoft YaHei',sans-serif">
    <div style="font-size:16px;color:#334155;line-height:1.7">
      你的好友 <b style="color:#0f172a">{inviter}</b> 邀请你加入
      <b style="color:#0f172a">{site}</b>。
    </div>
    <div style="font-size:14px;color:#475569;line-height:1.7;padding-top:8px">
      这是一封仅可使用一次的专属邀请，<b>{ttl}</b> 内有效。
      点击下方按钮完成注册：
    </div>
    <table role="presentation" cellpadding="0" cellspacing="0" \
style="margin:28px 0"><tr>
      <td style="border-radius:10px;background:#1e293b">
        <a href="{link}" style="display:inline-block;padding:14px 36px;\
font-size:15px;font-weight:700;color:#f8fafc;text-decoration:none">
          接受邀请 · 加入 {site}</a>
      </td>
    </tr></table>
    <div style="font-size:12px;color:#94a3b8;line-height:1.8">
      按钮点不开？复制此链接到浏览器：<br>
      <a href="{link}" style="color:#475569;word-break:break-all">{link}</a>
    </div>
    <div style="margin-top:24px;padding-top:20px;border-top:1px solid #e2e8f0;\
font-size:12px;color:#94a3b8;line-height:1.8">
      邀请码：<span style="font-family:ui-monospace,Consolas,monospace;\
color:#475569">{code}</span><br>
      如不认识 {inviter}，请忽略本邮件；此邀请码不会给你带来任何风险。
    </div>
  </div>
</td></tr>
<tr><td style="padding:16px 8px 0 8px;text-align:center;\
font-family:-apple-system,sans-serif;font-size:12px;color:#94a3b8">
  本邮件由 {site} 会员通过站内邀请功能发出 · {site_home}
</td></tr>
</table>
</td></tr></table>
</body></html>"#,
        inviter = esc(&l.inviter),
        site = esc(&l.site),
        logo = logo_html(l),
        link = esc(&l.link),
        code = esc(&l.code),
        ttl = ttl,
        site_home = esc(&home_of(&l.link)),
    )
}

/// 从注册链接提取主域（展示用「fluxtorrent.ptang.top」而非全链接）。
fn home_of(link: &str) -> String {
    link.split("//")
        .nth(1)
        .map(|rest| {
            let host = rest.split('/').next().unwrap_or(rest);
            host.to_string()
        })
        .unwrap_or_else(|| link.to_string())
}

/// 纯文本回落体：与 HTML 同信息量，写给纯文本客户端与邮件预览摘要。
fn plain_body(l: &InviteLetter) -> String {
    let tagline = l
        .tagline
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("一个热爱分享的资源社区");
    format!(
        "你的好友 {} 邀请你加入 {}（{}）！\n\n\
         打开以下链接注册（邀请码 {}内有效，仅可使用一次）：\n{}\n\n\
         邀请码：{}\n\n如不认识对方请忽略本邮件。",
        l.inviter,
        l.site,
        tagline,
        human_ttl(l.hours_left),
        l.link,
        l.code,
    )
}

/// 组装 multipart/alternative（HTML 优先 + 纯文本回落）。
/// 纯文本部分放前面、HTML 放后面是 RFC 2046 的推荐顺序（最简体在前）；
/// alternative_plain_html 收的是裸文本（内部自建 SinglePart）。
pub(crate) fn invite_multipart(l: &InviteLetter) -> MultiPart {
    MultiPart::alternative_plain_html(plain_body(l), html_body(l))
}

#[cfg(test)]
mod letter_tests {
    use super::*;

    fn letter() -> InviteLetter {
        InviteLetter {
            site: "FluxTorrent".into(),
            logo: None,
            tagline: Some("私有种子社区".into()),
            inviter: "老王<a>".into(),
            link: "https://fluxtorrent.ptang.top/register?invite=abc".into(),
            code: "abc123".into(),
            hours_left: 72,
        }
    }

    /// 用户名/站名进 HTML 必须转义——邀请人名字是用户可控文本。
    #[test]
    fn html_escapes_user_controlled_text() {
        let h = html_body(&letter());
        assert!(h.contains("老王&lt;a&gt;"), "username escaped");
        assert!(!h.contains("老王<a>"), "raw <a> must not appear");
    }

    /// TTL 人话：整 48h 以上且整除 → 天；否则小时。
    #[test]
    fn ttl_is_human_readable() {
        assert_eq!(human_ttl(72), "3 天");
        assert_eq!(human_ttl(30), "30 小时");
        assert_eq!(human_ttl(1), "1 小时");
    }

    /// 链接必须同时以可复制文本露出（按钮失效时的底线）。
    #[test]
    fn link_appears_as_copyable_text_too() {
        let h = html_body(&letter());
        let link = "https://fluxtorrent.ptang.top/register?invite=abc";
        assert!(h.contains(&format!(">{link}</a>")), "link as text");
    }

    /// 主域露出：页脚显示 host 而不是整条长链接。
    #[test]
    fn footer_shows_host_not_full_link() {
        assert_eq!(
            home_of("https://a.b.example/register?invite=x"),
            "a.b.example"
        );
    }

    /// 纯文本回落与 HTML 同信息量（链接/邀请码/邀请人都要在）。
    #[test]
    fn plain_fallback_carries_same_facts() {
        let p = plain_body(&letter());
        assert!(p.contains("abc123"));
        assert!(p.contains("老王<a>")); // 纯文本不转义
        assert!(p.contains("register?invite=abc"));
    }
}

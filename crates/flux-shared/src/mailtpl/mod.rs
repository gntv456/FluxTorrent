//! 统一邮件母版（玄墨装饰带 + 宣纸卡面 + 金线骨架）。
//!
//! api 与 worker 共用同一份骨架与 11 个内容函数：此前各自的发信口
//! 都是 `text/plain` 裸文本，且 api/worker 两处装配各有各的模板。
//! 收进本 crate 后，页脚/档位/语义色只有一处定义，两边引用同一份源码
//! （与 `apply_item_effect` 同一条纪律）。
//!
//! 约束（邮件客户端现实）：
//! - 表格布局 + 全内联样式，不引外部 CSS/Web 字体/图片/追踪像素；
//! - 装饰带内零文字（被剥背景时退化为一块留白，不会白底白字）；
//! - 所有用户可控文本进 HTML 前过 [`esc`]；纯文本部分不转义、不折 URL；
//! - 站内链接一律由调用方传入绝对基址（`mailer::site_base()`）。

mod templates;

pub use templates::*;

use std::fmt::Write as _;

/// 正文语言（与 `users.site_language` 的 en/chs/cht 一一对应）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    ZhCn,
    ZhTw,
    En,
}

impl Lang {
    /// 用户表已有的语言偏好字段；未知值按 zh-CN（站内缺省）处理。
    pub fn from_db(v: Option<&str>) -> Self {
        match v.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "en" => Lang::En,
            "cht" => Lang::ZhTw,
            _ => Lang::ZhCn,
        }
    }

    fn html_lang(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::En => "en",
        }
    }
}

/// 层级档位：层级靠留白（尺寸）表达，不靠换色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// 第一印象：邀请 / 找回密码 / 邮箱激活
    Hero,
    /// 日常通知：审核、申请、申诉结果
    Daily,
    /// 警示：账号状态 / H&R
    Alert,
}

impl Tier {
    fn band_height(self) -> u32 {
        match self {
            Tier::Hero => 96,
            Tier::Daily => 56,
            Tier::Alert => 64,
        }
    }
}

/// 语义色：只出现在装饰带底边线与印章/标签，不写正文。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// 无结论，纯品牌（邀请 / 找回密码）
    Gold,
    /// 待办 / 需继续动作
    Pending,
    /// 通过 / 成立
    Pass,
    /// 拒绝 / 驳回 / 封禁
    Reject,
    /// 预警 / 待处理
    Warn,
}

impl Tone {
    fn band_line(self) -> &'static str {
        match self {
            Tone::Gold => "#d3a13f",
            Tone::Pending => "#5cb3c7",
            Tone::Pass => "#3fae8f",
            Tone::Reject => "#d0442a",
            Tone::Warn => "#b07f1e",
        }
    }

    fn seal(self) -> &'static str {
        match self {
            Tone::Gold => "#a82e1c",
            Tone::Pending => "#23606f",
            Tone::Pass => "#1f6e59",
            Tone::Reject => "#a82e1c",
            Tone::Warn => "#6b5527",
        }
    }
}

/// 信息块一行；chip=true 时值用描边小标签（结论类）。
#[derive(Clone, Debug)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub chip: bool,
}

/// 纸色小卡片（结论块 / 安全兜底块 / 建议清单）。
#[derive(Clone, Debug)]
pub struct Block {
    pub heading: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct Cta {
    pub label: String,
    pub url: String,
}

/// 渲染输入：骨架只有这一份定义，11 封邮件只是字段不同。
#[derive(Clone, Debug)]
pub struct Content {
    pub lang: Lang,
    pub site: String,
    /// 站点绝对基址（无尾斜杠；空串=未配置，页脚链接退化为纯文本）
    pub base: String,
    pub tier: Tier,
    pub tone: Tone,
    /// 34×34 印章里的一个字
    pub seal: String,
    /// 印章右侧的金线小标签（如「种 子 审 核」）
    pub category: String,
    pub title: String,
    pub preheader: String,
    pub intro: String,
    pub result: Option<Block>,
    pub info: Vec<Row>,
    pub body: Vec<String>,
    pub bullets: Option<Block>,
    pub cta: Option<Cta>,
    pub cta_note: Option<String>,
    pub safety: Option<Block>,
    /// 页脚首句（必要通知说明）
    pub footer: String,
}

/// 一封待发邮件：主题 + 纯文本 part + HTML part + 发件人显示名。
#[derive(Clone, Debug)]
pub struct Mail {
    pub subject: String,
    pub text: String,
    pub html: String,
    pub site: String,
}

/// HTML 转义：`& < > " '`。
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// 主题行清洗：去 CR/LF/控制字符（防 header 注入）+ 长度上限 120 字。
/// 用户可控内容只进正文，不进 header；这里只是兜底。
pub fn clean_subject(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for (i, ch) in flat.chars().enumerate() {
        if i >= 120 {
            break;
        }
        out.push(ch);
    }
    out
}

/// 纯文本折行：宽度按字符计（中文 1 字=1 列）。
/// 整行是 URL 时不折——把长链接截断成两行会让收件人复制出错。
fn wrap_line(line: &str, width: usize, out: &mut Vec<String>) {
    let t = line.trim_end();
    if t.trim_start().starts_with("http://")
        || t.trim_start().starts_with("https://")
        || t.chars().count() <= width
    {
        out.push(t.to_string());
        return;
    }
    let mut cur = String::new();
    let mut cur_len = 0usize;
    for ch in t.chars() {
        if ch == '\n' {
            out.push(std::mem::take(&mut cur));
            cur_len = 0;
            continue;
        }
        cur.push(ch);
        cur_len += 1;
        if cur_len >= width {
            // 优先在最近的空格处断行（英文），否则按列宽硬断（中文）
            if let Some(pos) = cur.rfind(' ') {
                if pos > width / 3 {
                    let rest = cur[pos + 1..].to_string();
                    out.push(cur[..pos].trim_end().to_string());
                    cur = rest;
                    cur_len = cur.chars().count();
                    continue;
                }
            }
            out.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
}

/// 纯文本排版（68–72 字符宽；正文不转义、换行保留）。
pub fn render_text(c: &Content) -> String {
    const W: usize = 72;
    fn add(raw: &mut Vec<String>, s: &str) {
        for l in s.split('\n') {
            raw.push(l.trim_end().to_string());
        }
    }
    let mut raw: Vec<String> = Vec::new();
    add(&mut raw, &c.title);
    raw.push(String::new());
    add(&mut raw, &c.intro);
    if let Some(b) = &c.result {
        raw.push(String::new());
        add(&mut raw, &b.heading);
        add(&mut raw, &b.body);
    }
    if !c.info.is_empty() {
        raw.push(String::new());
        for r in &c.info {
            add(&mut raw, &format!("{}：{}", r.label, r.value));
        }
    }
    for p in &c.body {
        raw.push(String::new());
        add(&mut raw, p);
    }
    if let Some(b) = &c.bullets {
        raw.push(String::new());
        add(&mut raw, &b.heading);
        for (i, l) in b.body.split('\n').enumerate() {
            if !l.trim().is_empty() {
                add(&mut raw, &format!("{}. {}", i + 1, l.trim()));
            }
        }
    }
    if let Some(cta) = &c.cta {
        raw.push(String::new());
        add(&mut raw, &format!("{}：", cta.label));
        // 兜底段已含完整 URL 时不再重复印一遍
        let note_has_url =
            c.cta_note.as_ref().is_some_and(|n| n.contains(&cta.url));
        if !note_has_url {
            add(&mut raw, &cta.url);
        }
    }
    if let Some(n) = &c.cta_note {
        add(&mut raw, n);
    }
    if let Some(b) = &c.safety {
        raw.push(String::new());
        add(&mut raw, &b.heading);
        add(&mut raw, &b.body);
    }
    raw.push(String::new());
    add(&mut raw, &c.footer);
    add(&mut raw, &c.site);
    if c.base.is_empty() {
        raw.push(String::new());
    } else {
        add(
            &mut raw,
            &format!(
                "{0} · 站规 {0}/rules · 联系管理组 {0}/contactstaff",
                c.base
            ),
        );
    }
    let mut lines = Vec::new();
    for l in raw {
        wrap_line(&l, W, &mut lines);
    }
    format!("{}\n", lines.join("\n").trim_end())
}

fn chip_html(r: &Row, tone: Tone) -> String {
    if !r.chip {
        return format!(
            r#"<td style="padding:3px 0;font-size:14px;color:#3a453e;">{}</td>"#,
            esc(&r.value)
        );
    }
    let style = if tone == Tone::Gold {
        "background-color:#d3a13f;border:1px solid #8a6a1c;color:#241a05;"
            .to_string()
    } else {
        format!(
            "background-color:#faf6ec;border:1px solid {};color:{};",
            tone.seal(),
            tone.seal()
        )
    };
    format!(
        r#"<td style="padding:3px 0;font-size:14px;color:#3a453e;"><span style="display:inline-block;{}font-size:12px;font-weight:700;line-height:1;padding:5px 8px;border-radius:3px;">{}</span></td>"#,
        style,
        esc(&r.value)
    )
}

fn block_html(b: &Block) -> String {
    format!(
        r##"<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" bgcolor="#f2edde" style="margin:22px 0 0 0;background-color:#f2edde;border:1px solid #d3a13f;border-radius:6px;"><tr><td style="padding:14px 16px;"><div style="font-size:13px;font-weight:700;color:#232d27;line-height:1.5;">{}</div><p style="margin:6px 0 0 0;font-size:14px;line-height:1.8;color:#3a453e;">{}</p></td></tr></table>"##,
        esc(&b.heading),
        esc(&b.body).replace('\n', "<br>")
    )
}

const FONT: &str =
    "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'PingFang \
SC','Hiragino Sans GB','Microsoft YaHei','Noto Sans CJK SC',sans-serif";

/// 统一骨架渲染：11 封只有字段不同，结构永远这一段。
pub fn render_html(c: &Content) -> String {
    let mut s = String::with_capacity(12 * 1024);
    s.push_str("<!doctype html>\n<html lang=\"");
    s.push_str(c.lang.html_lang());
    s.push_str("\" dir=\"ltr\">\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str(
        "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <meta name=\"color-scheme\" content=\"light dark\">\n\
         <meta name=\"supported-color-schemes\" content=\"light dark\">\n",
    );
    let _ = write!(s, "<title>{}</title>\n", esc(&c.title));
    // 暗色模式钉住纸色，不换肤：避免「深色页面 + 客户端剥背景」变成浅字白底
    s.push_str(
        "<style>@media (prefers-color-scheme:dark){body{background-color:#f7f3e8\
!important;}.fx-card{background-color:#faf6ec!important;}.fx-bg{background-color:\
#f7f3e8!important;}}@media (max-width:480px){.fx-outer{padding:16px 8px!important\
;}.fx-pad{padding-left:20px!important;padding-right:20px!important;}.fx-h1{\
font-size:22px!important;}}</style>\n</head>\n",
    );
    s.push_str(
        "<body style=\"margin:0;padding:0;width:100%;background-color:#f7f3e8;\
-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;\">\n",
    );
    // 收件箱摘要（隐藏，与正文不重复）
    let _ = write!(
        s,
        "<div style=\"display:none;max-height:0;overflow:hidden;mso-hide:all;\
font-size:1px;line-height:1px;color:#f7f3e8;opacity:0;\">{}</div>\n",
        esc(&c.preheader)
    );
    s.push_str(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" \
cellspacing=\"0\" border=\"0\" class=\"fx-bg\" bgcolor=\"#f7f3e8\" \
style=\"background-color:#f7f3e8;background-image:repeating-linear-gradient(\
90deg,rgba(211,161,63,.09) 0 1px,rgba(211,161,63,0) 1px 7px);\"><tr>\
<td align=\"center\" class=\"fx-outer\" style=\"padding:32px 12px;\">\n\
<table role=\"presentation\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" \
border=\"0\" style=\"width:100%;max-width:600px;\">\n",
    );
    s.push_str(
        "<tr><td class=\"fx-card\" bgcolor=\"#faf6ec\" style=\"background-color:\
#faf6ec;border:1px solid #d3a13f;border-radius:6px;\">\n",
    );
    // 装饰带：零文字、零图片；bgcolor 兜底，#0a0f0d
    let h = c.tier.band_height();
    let _ = write!(
        s,
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" \
cellspacing=\"0\" border=\"0\"><tr><td class=\"fx-band\" height=\"{h}\" \
aria-hidden=\"true\" bgcolor=\"#0a0f0d\" style=\"height:{h}px;background-color:\
#0a0f0d;background-image:repeating-linear-gradient(90deg,rgba(211,161,63,.13) \
0 1px,rgba(211,161,63,0) 1px 18px),radial-gradient(200px 130px at 78% 4%,\
rgba(211,161,63,.18) 0%,rgba(211,161,63,0) 72%),linear-gradient(180deg,#15211a \
0%,#0a0f0d 58%,#101914 100%);border-bottom:1px solid {};font-size:0;\
line-height:0;\">&nbsp;</td></tr></table>\n",
        c.tone.band_line()
    );
    let _ = write!(
        s,
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" \
cellspacing=\"0\" border=\"0\"><tr><td class=\"fx-pad\" style=\"padding:30px \
32px 32px 32px;font-family:{FONT};\">\n"
    );
    // 品牌行（纯文字 + CSS 徽标，不放任何图片）
    let glyph = c.site.chars().next().unwrap_or('F').to_string();
    let _ = write!(
        s,
        "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" \
border=\"0\" width=\"100%\"><tr><td width=\"40\" valign=\"middle\" aria-hidden=\"true\" \
style=\"width:40px;\"><div style=\"width:34px;height:34px;line-height:32px;\
text-align:center;background-color:#0a0f0d;border:1px solid #d3a13f;\
border-radius:8px;font-size:16px;font-weight:700;color:#f7f3e8;\">{}</div>\
</td><td valign=\"middle\" style=\"padding-left:10px;\"><div style=\"font-size:\
16px;font-weight:700;line-height:1.3;color:#232d27;\">{}</div><div style=\
\"font-size:12px;line-height:1.4;color:#55605a;\">{}</div></td></tr></table>\n",
        esc(&glyph),
        esc(&c.site),
        esc(&c.category)
    );
    // 印章 + 金线标签
    let _ = write!(
        s,
        "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" \
border=\"0\" style=\"margin:22px 0 0 0;\"><tr><td width=\"34\" height=\"34\" \
align=\"center\" valign=\"middle\" aria-hidden=\"true\" bgcolor=\"#f7f3e8\" \
style=\"width:34px;height:34px;background-color:#f7f3e8;border:1px solid {};\
border-radius:3px;font-size:15px;font-weight:700;line-height:32px;color:{};\
\">{}</td><td style=\"padding-left:10px;font-size:12px;font-weight:700;color:\
#6b4a1a;letter-spacing:0.14em;\">{}</td></tr></table>\n",
        c.tone.seal(),
        c.tone.seal(),
        esc(&c.seal),
        esc(&c.category)
    );
    let _ = write!(
        s,
        "<h1 class=\"fx-h1\" style=\"margin:14px 0 8px 0;font-size:26px;\
line-height:1.35;font-weight:700;color:#232d27;\">{}</h1>\n\
<p style=\"margin:0;font-size:15px;line-height:1.75;color:#3a453e;\">{}</p>\n",
        esc(&c.title),
        esc(&c.intro)
    );
    if let Some(b) = &c.result {
        s.push_str(&block_html(b));
    }
    if !c.info.is_empty() {
        s.push_str(
            "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" \
cellspacing=\"0\" border=\"0\" bgcolor=\"#f1ecdd\" style=\"margin:22px 0 0 0;\
background-color:#f1ecdd;border:1px solid #d3a13f;border-radius:6px;\"><tr>\
<td style=\"padding:16px 18px;\"><table role=\"presentation\" width=\"100%\" \
cellpadding=\"0\" cellspacing=\"0\" border=\"0\">\n",
        );
        for r in &c.info {
            let _ = write!(
                s,
                "<tr><td width=\"76\" valign=\"top\" style=\"padding:3px 0;\
font-size:12px;color:#55605a;\">{}</td>{}</tr>\n",
                esc(&r.label),
                chip_html(r, c.tone)
            );
        }
        s.push_str("</table></td></tr></table>\n");
    }
    for p in &c.body {
        let _ = write!(
            s,
            "<p style=\"margin:18px 0 0 0;font-size:13px;line-height:1.8;\
color:#3a453e;\">{}</p>\n",
            esc(p)
        );
    }
    if let Some(b) = &c.bullets {
        let _ = write!(
            s,
            "<div style=\"margin:24px 0 0 0;font-size:13px;font-weight:700;\
color:#232d27;line-height:1.5;\">{}</div>\n<ol style=\"margin:8px 0 0 0;\
padding-left:20px;font-size:13px;line-height:1.9;color:#3a453e;\">\n",
            esc(&b.heading)
        );
        for l in b.body.split('\n') {
            if !l.trim().is_empty() {
                let _ = write!(s, "<li>{}</li>\n", esc(l.trim()));
            }
        }
        s.push_str("</ol>\n");
    }
    if let Some(cta) = &c.cta {
        let _ = write!(
            s,
            "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" \
border=\"0\" style=\"margin:26px 0 0 0;\"><tr><td bgcolor=\"#d3a13f\" \
align=\"center\" style=\"background-color:#d3a13f;border:1px solid #8a6a1c;\
border-radius:4px;padding:14px 30px;mso-padding-alt:14px 30px;\"><a href=\"{}\" \
style=\"font-family:{FONT};font-size:15px;font-weight:700;line-height:1;\
color:#241a05;text-decoration:none;display:inline-block;\">{}</a></td></tr>\
</table>\n",
            esc(&cta.url),
            esc(&cta.label)
        );
    }
    if let Some(n) = &c.cta_note {
        let _ = write!(
            s,
            "<p style=\"margin:18px 0 0 0;font-size:13px;line-height:1.8;\
color:#3a453e;\">{}</p>\n",
            esc(n)
        );
    }
    s.push_str(
        "<div style=\"margin:24px 0;border-top:1px solid #d3a13f;font-size:0;\
line-height:0;\">&nbsp;</div>\n",
    );
    if let Some(b) = &c.safety {
        s.push_str(&block_html(b));
    }
    s.push_str("</td></tr></table>\n</td></tr>\n");
    // 页脚
    let _ = write!(
        s,
        "<tr><td style=\"padding:18px 8px 0 8px;text-align:center;font-family:\
{FONT};\"><p style=\"margin:0;font-size:12px;line-height:1.9;color:#55605a;\">\
{}</p>\n",
        esc(&c.footer)
    );
    if c.base.is_empty() {
        let _ = write!(
            s,
            "<p style=\"margin:4px 0 0 0;font-size:12px;line-height:1.9;\
color:#55605a;\">{}</p>\n",
            esc(&c.site)
        );
    } else {
        let _ = write!(
            s,
            "<p style=\"margin:4px 0 0 0;font-size:12px;line-height:1.9;\
color:#55605a;\"><a href=\"{0}\" style=\"color:#1f6e59;text-decoration:underline;\
\">{1}</a> · <a href=\"{0}/rules\" style=\"color:#1f6e59;text-decoration:\
underline;\">站规</a> · <a href=\"{0}/contactstaff\" style=\"color:#1f6e59;\
text-decoration:underline;\">联系管理组</a></p>\n",
            esc(&c.base),
            esc(&c.site)
        );
    }
    s.push_str("</td></tr>\n</table>\n</td></tr></table>\n</body>\n</html>\n");
    s
}

/// 由内容生成一封可直接投递的邮件（主题清洗 + 双 part）。
pub fn build_content(subject: &str, c: Content) -> Mail {
    Mail {
        subject: clean_subject(subject),
        text: render_text(&c),
        html: render_html(&c),
        site: c.site.clone(),
    }
}

/// outbox 单列存不下第二个 part：把 text/html 装进一个带前缀的 JSON。
/// 老行没有前缀 ⇒ 按纯文本投递，行为不变；新行 worker 解出 HTML 后发
/// `multipart/alternative`。不改表结构（不新增迁移）。
pub const ENVELOPE_PREFIX: &str = "\u{1}FLUXMAIL2 ";

pub fn envelope(text: &str, html: &str) -> String {
    let v = serde_json::json!({ "t": text, "h": html });
    format!("{}{}", ENVELOPE_PREFIX, v)
}

/// 还原 outbox 正文：返回 (text, html)；html 为空表示这是一封历史纯文本。
pub fn open_envelope(body: &str) -> (String, String) {
    let Some(rest) = body.strip_prefix(ENVELOPE_PREFIX) else {
        return (body.to_string(), String::new());
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(rest) else {
        return (body.to_string(), String::new());
    };
    let text = v.get("t").and_then(|x| x.as_str()).unwrap_or("");
    let html = v.get("h").and_then(|x| x.as_str()).unwrap_or("");
    (text.to_string(), html.to_string())
}

#[cfg(test)]
mod tests;

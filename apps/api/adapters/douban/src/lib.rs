// 生态商店 M4 自营适配器：douban（元数据源）——SDK 版。
// 与 adapters/sdk.rs（宿主 ABI 绑定层）配合，业务逻辑只剩 parse 一层。
// 构建：cargo build --target wasm32-unknown-unknown --release
//
// 正式化（2026-09-24）：桌面页被 302+JS 挑战拦截；m.douban.com 移动端
// 直出 og: 元数据。入口把 movie.douban.com/subject/N 重写为
// m.douban.com/movie/subject/N（manifest.http_allow 需同时覆盖两域——
// 宿主按重写后的 URL 校验白名单）。

#![no_std]

#[path = "../../sdk.rs"]
mod sdk;

use sdk::{http_fetch, leak, log_warn};

sdk::entry!(|url: &str| -> &'static str {
    let rewritten = rewrite(url);
    let html = match http_fetch(rewritten) {
        Ok(h) => h,
        Err(e) => {
            log_warn(&sdk::leak(concat_err(e)));
            return leak("{\"success\":false,\"error\":\"fetch\"}");
        }
    };
    let (title, poster, descr) = parse(html);
    leak(build_json(title, poster, descr).as_str())
});

fn concat_err(e: &str) -> &'static str {
    let mut s = MiniString::new();
    s.push_str("douban fetch failed: ");
    s.push_str(e);
    sdk::leak(s.as_str())
}

/// 桌面条目 URL → 移动端（JS 挑战规避）；非 movie.douban 原样
fn rewrite(url: &str) -> &str {
    if let Some(rest) = url.strip_prefix("https://movie.douban.com/") {
        let mut s = MiniString::new();
        s.push_str("https://m.douban.com/");
        s.push_str(rest);
        return sdk::leak(s.as_str());
    }
    url
}

/// HTML 实体最小解码（og: 属性值里 &amp; 常见）
fn decode_entities(v: &str) -> MiniString {
    let mut s = MiniString::new();
    let mut rest = v;
    while let Some(i) = rest.find('&') {
        s.push_str(&rest[..i]);
        let (ent, ch, len) = if rest[i..].starts_with("&amp;") {
            ("&amp;", '&', 5)
        } else if rest[i..].starts_with("&quot;") {
            ("&quot;", '"', 6)
        } else if rest[i..].starts_with("&#39;") {
            ("&#39;", '\'', 5)
        } else if rest[i..].starts_with("&lt;") {
            ("&lt;", '<', 4)
        } else if rest[i..].starts_with("&gt;") {
            ("&gt;", '>', 4)
        } else {
            // 未知实体：原样吐出 &
            ("", '&', 1)
        };
        let _ = ent;
        s.push(ch);
        rest = &rest[i + len..];
    }
    s.push_str(rest);
    s
}

fn parse(html: &str) -> (&str, &str, &str) {
    // m.douban og:title 形如「肖申克的救赎 - 电影」：剥尾部类型
    let raw_title = extract_between(
        html,
        "property=\"og:title\" content=\"",
        "\"",
    )
    .unwrap_or("");
    let title = strip_type_suffix(raw_title);
    let poster = extract_between(
        html,
        "property=\"og:image\" content=\"",
        "\"",
    )
    .unwrap_or("");
    let descr = extract_between(
        html,
        "property=\"og:description\" content=\"",
        "\"",
    )
    .map(|d| d.trim())
    .filter(|d| !d.is_empty())
    .unwrap_or("");
    (title, poster, descr)
}

/// 「名 - 电影/电视剧/…」→「名」（无尾部则原样）
fn strip_type_suffix(t: &str) -> &str {
    let t = t.trim();
    for sep in [" - 电影", " - 电视剧", " - 影人", " - 音乐"] {
        if let Some(base) = t.strip_suffix(sep) {
            return base.trim();
        }
    }
    t
}

fn extract_between<'a>(
    html: &'a str,
    open: &str,
    close: &str,
) -> Option<&'a str> {
    let i = html.find(open)? + open.len();
    let j = html[i..].find(close)? + i;
    Some(&html[i..j])
}

fn build_json(title: &str, poster: &str, descr: &str) -> MiniString {
    let mut s = MiniString::new();
    s.push_str("{\"success\":true,\"source\":\"douban\",\"name\":\"");
    push_escaped(&mut s, decode_entities(title).as_str(), 512);
    s.push_str("\",\"poster\":\"");
    push_escaped(&mut s, decode_entities(poster).as_str(), 1024);
    s.push_str("\",\"descr\":\"");
    push_escaped(&mut s, decode_entities(descr).as_str(), 8000);
    s.push_str("\"}");
    s
}

/// no_std 定长字符串（JSON 拼装）
pub(crate) struct MiniString {
    buf: [u8; 16 * 1024],
    len: usize,
}

impl MiniString {
    pub(crate) fn new() -> Self {
        MiniString { buf: [0; 16 * 1024], len: 0 }
    }
    pub(crate) fn push_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }
    pub(crate) fn push(&mut self, c: char) {
        let mut tmp = [0u8; 4];
        self.push_str(c.encode_utf8(&mut tmp));
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn truncate(&mut self, n: usize) {
        if n < self.len {
            self.len = n;
        }
    }
    pub(crate) fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

fn push_escaped(s: &mut MiniString, v: &str, cap: usize) {
    let mut n = 0;
    for c in v.chars() {
        if n >= cap {
            break;
        }
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' | '\r' | '\t' => s.push_str(" "),
            c if (c as u32) >= 0x20 => {
                s.push_str(c.encode_utf8(&mut [0u8; 4]))
            }
            _ => {}
        }
        n += c.len_utf8();
    }
    s.truncate(s.len().min(cap + 8));
}

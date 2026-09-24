// 生态商店 M4 自营适配器：douban（元数据源）——SDK 版。
// 与 adapters/sdk.rs（宿主 ABI 绑定层）配合，业务逻辑只剩 parse 一层。
// 构建：cargo build --target wasm32-unknown-unknown --release

#![no_std]

#[path = "../../sdk.rs"]
mod sdk;

use sdk::entry;

use sdk::{http_fetch, leak, log_warn};

entry!(|url: &str| -> &'static str {
    let html = match http_fetch(url) {
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
    // no_std 无 format!：拼接固定前缀
    let mut s = MiniString::new();
    s.push_str("douban fetch failed: ");
    s.push_str(e);
    sdk::leak(s.as_str())
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
    pub(crate) fn truncate(&mut self, n: usize) {
        if n < self.len {
            self.len = n;
        }
    }
    pub(crate) fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

fn build_json(title: &str, poster: &str, descr: &str) -> MiniString {
    let mut s = MiniString::new();
    s.push_str("{\"success\":true,\"source\":\"douban\",\"name\":\"");
    push_escaped(&mut s, title, 512);
    s.push_str("\",\"poster\":\"");
    push_escaped(&mut s, poster, 1024);
    s.push_str("\",\"descr\":\"");
    push_escaped(&mut s, descr, 8000);
    s.push_str("\"}");
    s
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

fn parse(html: &str) -> (&str, &str, &str) {
    let title = extract_between(html, "<title>", "</title>")
        .map(|t| t.trim_end_matches(" (豆瓣)").trim())
        .filter(|t| !t.is_empty())
        .unwrap_or("");
    let poster =
        extract_between(html, "rel=\"v:image\" href=\"", "\"").unwrap_or("");
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

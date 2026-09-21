//! 原始 query string 字节层参数提取。
//! 从 helpers.rs 按域拆出。

use crate::peers::percent_decode;

/// 从原始 query string 提取参数（字节层，percent-decode）
pub(crate) struct RawParams<'a> {
    raw: &'a str,
}

impl<'a> RawParams<'a> {
    pub(crate) fn new(raw: &'a str) -> Self {
        Self { raw }
    }
    /// 返回解码后的字节值
    pub(crate) fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        for pair in self.raw.split('&') {
            let mut it = pair.splitn(2, '=');
            if it.next() == Some(key) {
                let v = it.next().unwrap_or("");
                return Some(percent_decode(v.as_bytes()));
            }
        }
        None
    }
    pub(crate) fn get_str(&self, key: &str) -> Option<String> {
        self.get_bytes(key)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }
    pub(crate) fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get_str(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

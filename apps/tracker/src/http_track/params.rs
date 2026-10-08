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
    /// 严格取值：`None`=参数缺失，`Some(Err)`=有值但不是合法 i64（非数字、
    /// 超 i64 都算）。审计 10-07 P2-10：10-06 把 `left` 改成严格解析，但
    /// `uploaded/downloaded/port` 仍走 `get_i64` 的「解析失败回落默认值」，
    /// 于是 `uploaded=99999999999999999999999` 被当成 0 —— 客户端读数被清空
    /// 还会被 ledger_guard 判成「读数回退」而误记 counter_reset 作弊留痕。
    pub(crate) fn get_i64_strict(&self, key: &str) -> Option<Result<i64, ()>> {
        self.get_str(key)
            .map(|v| v.trim().parse::<i64>().map_err(|_| ()))
    }
    /// announce 的三个数值参数（port / uploaded / downloaded）一次解析完。
    /// 缺失按 0（BEP3 允许 stopped 不带 port），有值但非法一律回 bencode 失败。
    pub(crate) fn announce_nums(
        &self,
    ) -> Result<(u16, i64, i64), actix_web::HttpResponse> {
        let port_raw = self.strict_i64("port", 0, "port 无效")?;
        if !(0..=65535).contains(&port_raw) {
            return Err(super::emit::bencode_err("port 无效"));
        }
        let up = self.strict_i64("uploaded", 0, "uploaded/downloaded 无效")?;
        let down =
            self.strict_i64("downloaded", 0, "uploaded/downloaded 无效")?;
        if up < 0 || down < 0 {
            return Err(super::emit::bencode_err("uploaded/downloaded 无效"));
        }
        Ok((port_raw as u16, up, down))
    }

    /// 严格取一个整型参数：缺失按 `absent`，有值但非法（非数字/超 i64）直接返回
    /// bencode 失败响应。10-07 P2-10：`get_i64` 的「解析失败回落默认值」会把
    /// `uploaded=9999…` 静默当 0，清空客户端累计读数并误触 counter_reset 留痕。
    pub(crate) fn strict_i64(
        &self,
        key: &str,
        absent: i64,
        msg: &'static str,
    ) -> Result<i64, actix_web::HttpResponse> {
        match self.get_i64_strict(key) {
            None => Ok(absent),
            Some(Ok(v)) => Ok(v),
            Some(Err(_)) => Err(super::emit::bencode_err(msg)),
        }
    }
    /// `event` 是不是 stopped（缺省视为非 stopped）
    pub(crate) fn is_stopped(&self) -> bool {
        self.get_str("event").is_some_and(|v| v == "stopped")
    }
    pub(crate) fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get_str(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
    /// 取出**全部**同名参数值（BEP3 允许重复 key；交叉上报的 `xreport`
    /// 可能被客户端拆成多个 `xreport=` 段）。
    pub(crate) fn get_all(&self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        for pair in self.raw.split('&') {
            let mut it = pair.splitn(2, '=');
            if it.next() == Some(key) {
                let v = it.next().unwrap_or("");
                let decoded = percent_decode(v.as_bytes());
                out.push(String::from_utf8_lossy(&decoded).into_owned());
            }
        }
        out
    }
}

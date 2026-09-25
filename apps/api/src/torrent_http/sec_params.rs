//! 多维筛选参数解析（B3，2026-09-25）。
//!
//! 从 `list.rs` 拆出（守 300 行）。`sec_{kind}` 这类参数**不能**走 `ListQuery`
//! 的反序列化——维度是站长自建的，字段名在编译期未知，只能扫原始 query string。
//!
//! 写法：
//! ```text
//!   sec_{kind}=值            多值逗号/重复参数（枚举=dict_id，bool=true|false，
//!                            text=关键词，number/date=精确值）
//!   sec_{kind}_min= / _max=  数字/日期区间（含端点）
//! ```
//! 前缀歧义：kind 允许下划线，`sec_x_min` 既可能是 kind=`x_min`，也可能是
//! kind=`x` 的下界。规则：**整段能对上 section_kinds 就优先当维度名**，
//! 否则再剥离 `_min`/`_max`。

use sqlx::PgPool;

use crate::torrents::section_filter::{build_section_filters, SectionFilter};

/// 从原始 query string 解析 `sec_*` 参数并归并成筛选条件。
/// 前台列表与后台管理列表共用（`pub(crate)`）——两处筛选语义必须一致。
pub(crate) async fn parse_section_params(
    db: &PgPool,
    query: &str,
) -> Vec<SectionFilter> {
    // (kind, op, value)：op ∈ eq / min / max
    let mut raw: Vec<(String, String, String)> = Vec::new();
    for pair in query.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let Some(rest) = k.strip_prefix("sec_") else {
            continue;
        };
        if rest.is_empty() || v.is_empty() {
            continue;
        }
        let decoded = urldecode(v);
        if is_kind(db, rest).await {
            // 整段就是维度名：多值（逗号串 / 重复参数）逐个收集
            for part in decoded.split(',') {
                let t = part.trim();
                if !t.is_empty() {
                    raw.push((rest.into(), "eq".into(), t.into()));
                }
            }
            continue;
        }
        // 再试 `_min` / `_max` 后缀
        if let Some(base) = rest.strip_suffix("_min") {
            if is_kind(db, base).await {
                raw.push((base.into(), "min".into(), decoded.trim().into()));
            }
        } else if let Some(base) = rest.strip_suffix("_max") {
            if is_kind(db, base).await {
                raw.push((base.into(), "max".into(), decoded.trim().into()));
            }
        }
    }
    build_section_filters(db, raw).await
}

async fn is_kind(db: &PgPool, kind: &str) -> bool {
    crate::admin_p3_http::is_custom_kind(db, kind).await
}

/// 百分号解码：值直接从原始 query string 取，actix 不解码。
/// `+` 按空格处理（与表单编码一致）。非法 `%XX` 原样保留，不报错。
fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(c) => {
                        out.push(c);
                        i += 3;
                    }
                    None => {
                        out.push(b[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::urldecode;

    #[test]
    fn decodes_percent_and_plus() {
        assert_eq!(urldecode("a%20b"), "a b");
        assert_eq!(urldecode("a+b"), "a b");
        assert_eq!(urldecode("caf%C3%A9"), "café");
        // 非法 %XX 原样保留，不 panic
        assert_eq!(urldecode("100%"), "100%");
        assert_eq!(urldecode("%zz"), "%zz");
    }
}

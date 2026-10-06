//! 发种参数装配（0288 从 upload_precheck.rs 拆出，300 行门禁）。
//!
//! 元数据两条通道都收：query string（老客户端与 `/open/torrents` 在用）与
//! multipart 文本字段（浏览器发种页改走这条）。

use crate::errors::{DomainError, DomainResult};

use super::ptgen::UploadForm;

/// 发种元数据的字段类型表（multipart 文本与 query string 都是字符串，
/// 必须先还原成 `UploadForm` 声明的类型，否则 serde 会把 "1" 当字符串拒收）。
const NUMERIC_FIELDS: [&str; 8] = [
    "category_id",
    "medium_id",
    "grade_id",
    "edition_id",
    "price",
    "group_id",
    "pos_state",
    "pick_type",
];
const BOOL_FIELDS: [&str; 1] = ["anonymous"];

/// 组装 `UploadForm`：query string 与 multipart 文本字段**都收**，后者优先。
///
/// 为什么要 multipart 这条通道（P1-3 实测）：描述走 query string 时，实际上限由
/// HTTP 请求行决定而不是本站业务规则——8192 汉字的简介直接 400（且响应体为空、
/// 无任何信息），16384 字 431；而前端 textarea 的 maxLength 是 30000，
/// 等于「前端允许的长度是后端能收的两三倍」，发种员只看到一句网络错误。
pub(super) fn build_form(
    query_string: &str,
    mp_fields: Vec<(String, String)>,
) -> DomainResult<UploadForm> {
    use serde_json::{Map, Value};
    let mut map: Map<String, Value> = Map::new();
    for (k, v) in url::form_urlencoded::parse(query_string.as_bytes()) {
        map.insert(k.into_owned(), Value::String(v.into_owned()));
    }
    for (k, v) in mp_fields {
        map.insert(k, Value::String(v));
    }
    // 字符串 → 声明类型（空串视作未填写）
    for key in NUMERIC_FIELDS {
        if let Some(Value::String(raw)) = map.get(key) {
            let t = raw.trim();
            if t.is_empty() {
                map.insert(key.into(), Value::Null);
                continue;
            }
            let n: i64 = t.parse().map_err(|_| {
                DomainError::Validation(format!("{key} 需为整数（当前 {raw}）"))
            })?;
            map.insert(key.into(), Value::from(n));
        }
    }
    for key in BOOL_FIELDS {
        if let Some(Value::String(raw)) = map.get(key) {
            let on = matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            );
            map.insert(key.into(), Value::Bool(on));
        }
    }
    serde_json::from_value(Value::Object(map))
        .map_err(|e| DomainError::Validation(format!("发种参数非法: {e}")))
}

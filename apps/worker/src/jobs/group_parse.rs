//! Redis stream 回复解析（从 group.rs 拆出，同受 300 行门禁约束）。
//!
//! 只碰 `redis::Value` 这一层未类型化的原始回复：XREADGROUP / XAUTOCLAIM
//! 的嵌套数组形态在 redis 0.27 + Redis 6.2/7 之间有差异，解析规则集中
//! 在此便于单测，业务侧（consume_*）只看到 (id, payload) 对。

use redis::Value;

/// 解析 XREADGROUP 的 redis::Value（nil / [[stream, entries]..]）。
pub(super) fn parse_stream_reply(v: &Value) -> Vec<(String, String)> {
    match v {
        Value::Array(items) => items
            .iter()
            .filter_map(|se| match se {
                Value::Array(pair) if pair.len() >= 2 => {
                    parse_entries(&pair[1])
                }
                _ => None,
            })
            .flatten()
            .collect(),
        _ => Vec::new(),
    }
}

/// 解析 XAUTOCLAIM 回复——entries 都在下标 1：Redis 7 为
/// [next-id, entries, deleted] 三元，6.2 为 [next-id, entries] 二元
/// （两种形态兼容；当前 compose 固定 redis:7）。
pub(super) fn parse_autoclaim_reply(v: &Value) -> Vec<(String, String)> {
    match v {
        Value::Array(items) if items.len() >= 2 => {
            parse_entries(&items[1]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// 条目列表 → (id, payload)；无 payload 字段的损坏条目返回 ("", "") 由
/// 调用方按 DLQ 处理。
fn parse_entries(v: &Value) -> Option<Vec<(String, String)>> {
    let Value::Array(entries) = v else {
        return None;
    };
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let Value::Array(pair) = e else {
            continue;
        };
        if pair.len() < 2 {
            continue;
        }
        let id = value_to_string(&pair[0]);
        let payload = flat_fields(&pair[1])
            .into_iter()
            .find(|(k, _)| k == "payload")
            .map(|(_, v)| v)
            .unwrap_or_default();
        out.push((id, payload));
    }
    Some(out)
}

/// XID 在 redis 0.27 里可能是 BulkString("ms-seq") 或 Bulk([ms, seq])
fn value_to_string(v: &Value) -> String {
    match v {
        Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
        Value::Array(parts) => parts
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("-"),
        other => format!("{other:?}"),
    }
}

/// field-value 扁平列表 → (k, v) 对
fn flat_fields(v: &Value) -> Vec<(String, String)> {
    let Value::Array(items) = v else {
        return Vec::new();
    };
    items
        .chunks(2)
        .filter_map(|c| match (&c[0], c.get(1)) {
            (
                Value::BulkString(k),
                Some(Value::BulkString(val)),
            ) => Some((
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(val).to_string(),
            )),
            _ => None,
        })
        .collect()
}

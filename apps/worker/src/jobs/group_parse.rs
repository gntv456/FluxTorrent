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
            (Value::BulkString(k), Some(Value::BulkString(val))) => Some((
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(val).to_string(),
            )),
            _ => None,
        })
        .collect()
}

/// XTRIM 的安全下界，由 XPENDING 摘要判出（消费方见 `group::trim_safe`）。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TrimBound {
    /// 组内没有未确认条目 ⇒ 可以按长度裁，留一段历史供排障
    NoPending,
    /// 最旧未确认条目 id ⇒ 只允许裁掉它之前的条目
    Oldest(String),
    /// 回复读不出结论 ⇒ 本轮不裁（宁可涨长度，也不丢未入账的事件）
    Unknown,
}

/// 解析 `XPENDING <stream> <group>` 摘要
/// `[count, min-id, max-id, [[consumer, pending]..]]`。
/// count>0 而 min-id 是 Nil 属于读不出结论，落 `Unknown`。
pub(super) fn parse_xpending_reply(v: &Value) -> TrimBound {
    let Value::Array(rows) = v else {
        return TrimBound::Unknown;
    };
    if rows.len() < 3 {
        return TrimBound::Unknown;
    }
    let count: i64 = match &rows[0] {
        Value::Int(n) => *n,
        other => value_to_string(other).parse().unwrap_or(-1),
    };
    if count < 0 {
        return TrimBound::Unknown;
    }
    if count == 0 {
        return TrimBound::NoPending;
    }
    if matches!(&rows[1], Value::Nil) {
        return TrimBound::Unknown;
    }
    TrimBound::Oldest(value_to_string(&rows[1]))
}

#[cfg(test)]
mod trim_tests {
    use super::*;

    fn arr(items: Vec<Value>) -> Value {
        Value::Array(items)
    }

    fn bulk(s: &str) -> Value {
        Value::BulkString(s.as_bytes().to_vec())
    }

    #[test]
    fn zero_pending_allows_length_trim() {
        let v = arr(vec![Value::Int(0), Value::Nil, Value::Nil, arr(vec![])]);
        assert_eq!(parse_xpending_reply(&v), TrimBound::NoPending);
    }

    #[test]
    fn pending_reports_oldest_id() {
        let v = arr(vec![
            Value::Int(7),
            bulk("1700000000001-0"),
            bulk("1700000000009-0"),
            arr(vec![]),
        ]);
        assert_eq!(
            parse_xpending_reply(&v),
            TrimBound::Oldest("1700000000001-0".into())
        );
    }

    #[test]
    fn count_as_string_still_parses() {
        // RESP2 下 count 可能是整数字节串
        let v = arr(vec![bulk("3"), bulk("10-0"), bulk("12-0"), arr(vec![])]);
        assert_eq!(parse_xpending_reply(&v), TrimBound::Oldest("10-0".into()));
    }

    #[test]
    fn unreadable_replies_never_trim() {
        // 有 pending 却读不到 min-id：不能当成「无 pending」去盲裁
        let v = arr(vec![Value::Int(2), Value::Nil, Value::Nil, arr(vec![])]);
        assert_eq!(parse_xpending_reply(&v), TrimBound::Unknown);
        assert_eq!(parse_xpending_reply(&Value::Nil), TrimBound::Unknown);
        assert_eq!(
            parse_xpending_reply(&arr(vec![Value::Int(1)])),
            TrimBound::Unknown
        );
    }
}

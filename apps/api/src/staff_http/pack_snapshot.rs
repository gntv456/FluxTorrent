//! 站型包「另存快照」的载荷采集（四审 L2 P0）。
//!
//! 预置包用 sections/tags/classes/economy/metadata 五段 JSONB 携带自定义内容，
//! 另存时必须按同形采集：缺段会让 custom 包 apply 时把站长已配的维度、标签词表、
//! 等级叙事、经济预设当成「本包未声明」而整体跳过——四审前正是这条路径丢数据。

use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

pub(super) struct PackSnapshot {
    pub(super) sections: Value,
    pub(super) tags: Value,
    pub(super) classes: Value,
    pub(super) economy: Value,
    pub(super) metadata: Value,
}

/// 采集当前站点的五段自定义载荷。NULL 一律表达「未声明」，交由 apply 侧的
/// `pack_declares_sections` 等守卫决定不动哪些维度，绝不主动清空。
pub(super) async fn collect(db: &PgPool) -> DomainResult<PackSnapshot> {
    let kinds: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT kind, label, sort FROM section_kinds ORDER BY sort, kind",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let dict_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, name FROM section_dict ORDER BY sort, id")
            .fetch_all(db)
            .await
            .unwrap_or_default();

    let sections = if kinds.is_empty() {
        Value::Null
    } else {
        let mut dict: Map<String, Value> = Map::new();
        for (kind, name) in &dict_rows {
            dict.entry(kind.clone())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("entry 只会是数组")
                .push(Value::String(name.clone()));
        }
        // 空选项的维度也要显式列出（与预置包的 "team": [] 同形）
        for (kind, _, _) in &kinds {
            dict.entry(kind.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
        }
        json!({
            "kinds": kinds
                .iter()
                .map(|(kind, label, sort)| {
                    json!({"kind": kind, "label": label, "sort": sort})
                })
                .collect::<Vec<_>>(),
            "dict": dict,
        })
    };

    let tag_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, kind, COALESCE(tag_group, 'attribute') FROM tag_dict \
         WHERE scope = 'torrent' AND scope_layer = 'pack' ORDER BY sort, id",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let tags = Value::Array(
        tag_rows
            .into_iter()
            .map(|(name, kind, group)| {
                json!({"name": name, "kind": kind, "group": group})
            })
            .collect(),
    );

    let class_rows: Vec<(i32, String)> = sqlx::query_as(
        "SELECT class_id, name FROM class_rules ORDER BY class_id",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let classes = if class_rows.is_empty() {
        Value::Null
    } else {
        Value::Array(
            class_rows
                .into_iter()
                .map(|(id, name)| json!({"id": id, "name": name}))
                .collect(),
        )
    };

    // 经济预设的键集跟随预置包：只采「预置包 economy 里出现过的键」，
    // 既不会把 module_* 开关塞进来，也无需在此重复一份白名单。
    let econ_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT s.name, s.value FROM site_settings s \
         WHERE s.name NOT LIKE 'module\\_%' AND s.name IN \
         (SELECT jsonb_object_keys(economy) FROM site_type_packs \
          WHERE economy IS NOT NULL)",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let economy = if econ_rows.is_empty() {
        Value::Null
    } else {
        Value::Object(
            econ_rows
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect(),
        )
    };

    let sources: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'metadata_sources'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let metadata = match sources {
        Some(v) if !v.trim().is_empty() => json!({
            "sources": v
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| Value::String(s.to_string()))
                .collect::<Vec<_>>()
        }),
        _ => Value::Null,
    };

    Ok(PackSnapshot {
        sections,
        tags,
        classes,
        economy,
        metadata,
    })
}

//! 站型包「另存快照」的载荷采集（四审 L2 P0）。
//!
//! 预置包用 sections/tags/classes/economy/metadata 五段 JSONB 携带自定义内容，
//! 另存时必须按同形采集：缺段会让 custom 包 apply 时把站长已配的维度、标签词表、
//! 等级叙事、经济预设当成「本包未声明」而整体跳过——四审前正是这条路径丢数据。
//!
//! G21 起本模块还承担 apply 台账的两件事：diff 预览（回看，与向导同源）与
//! apply 前全量状态快照（回滚基线，回滚 = 把快照当一次 apply 执行）。

use super::sitetype::SiteTypePack;
use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

pub(super) struct PackSnapshot {
    pub(super) sections: Value,
    pub(super) tags: Value,
    pub(super) classes: Value,
    pub(super) economy: Value,
    pub(super) metadata: Value,
    /// 术语规则（0206）：与 tags 同样「总写成数组」，另存即如实捕获当前站点的
    /// 词汇表（空数组 = 本站确实没有规则）。apply 侧 NULL 才是「未声明、不动」。
    pub(super) terms: Value,
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

    let terms: Vec<(String, String, bool, i32)> = sqlx::query_as(
        "SELECT canonical, replacement, enabled, sort FROM site_terms \
         ORDER BY sort, canonical",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    // 与 tags 同形：总写数组，空站就是空数组（NULL 留给「本包不声明术语」用）。
    // `enabled` 必须一起采：只停用语义丢了，apply 回去就等于擅自把站长关掉的
    // 规则全部重新打开——正是 0193/0197 那批「另存丢载荷」的复发病形。
    let terms = Value::Array(
        terms
            .into_iter()
            .map(|(canonical, replacement, enabled, sort)| {
                json!({
                    "canonical": canonical,
                    "replacement": replacement,
                    "enabled": enabled,
                    "sort": sort,
                })
            })
            .collect(),
    );

    Ok(PackSnapshot {
        sections,
        tags,
        classes,
        economy,
        metadata,
        terms,
    })
}

/// 站型包 apply 的 diff 预览（U2 §8.2 / G21 回看）：apply 将改动的键旧值→新值，
/// 不落库。向导端点与 apply 台账共用同一实现，保证「预览所见 = 记录所存」。
/// 返回 (未变动项数, changes)。
///
/// H2（0317）：除 site_type/site_name/module_* 外还含**分类段**——
/// 改名（同 key/同 id 命中且名字变化，带在用种子数）/ 新增 / 失活
/// （现存但包未声明的分类）。分类语义偷换此前在预览里完全不可见。
pub(super) async fn diff_preview(
    db: &PgPool,
    pack: &SiteTypePack,
) -> DomainResult<(usize, Vec<Value>)> {
    let current: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('site_type','site_name') OR name LIKE 'module\\_%'",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let cur = std::collections::HashMap::<String, String>::from_iter(current);
    let mut changes: Vec<Value> = Vec::new();
    let mut push = |key: &str, old: Option<&String>, new: &str| {
        let old_v = old.map(|s| s.as_str()).unwrap_or("(未设置)");
        if old_v != new {
            changes.push(json!({ "key": key, "old": old_v, "new": new }));
        }
    };
    push("site_type", cur.get("site_type"), &pack.code);
    // H3 对齐：brand 为空时 apply 不再写 site_name，预览同样不虚报「站名将被清空」
    if !pack.brand.trim().is_empty() {
        push("site_name", cur.get("site_name"), pack.brand.trim());
    }
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let setting = format!("module_{k}");
            let new = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            push(&setting, cur.get(&setting), new);
        }
    }
    // ---- 分类段（H2）----
    let cat_rows: Vec<(i32, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT c.id, c.key, c.name, (SELECT count(*) FROM torrents t \
         WHERE t.category_id = c.id) FROM categories c",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    if let Some(cats) = pack.categories.as_array() {
        let declared_keys: std::collections::HashSet<&str> = cats
            .iter()
            .filter_map(|c| c.get("key").and_then(Value::as_str))
            .collect();
        let declared_ids: std::collections::HashSet<i64> = cats
            .iter()
            .filter_map(|c| c.get("id").and_then(Value::as_i64))
            .collect();
        for c in cats {
            let Some(new_name) = c.get("name").and_then(Value::as_str) else {
                continue;
            };
            let pkey = c.get("key").and_then(Value::as_str);
            let pid = c.get("id").and_then(Value::as_i64);
            let hit = cat_rows.iter().find(|(id, k, _, _)| {
                pkey.is_some_and(|pk| k.as_deref() == Some(pk))
                    || (pkey.is_none()
                        && pid.is_some_and(|pi| *id as i64 == pi))
            });
            match hit {
                Some((_, _, old_name, torrents)) if old_name != new_name => {
                    changes.push(json!({
                        "key": "category_rename",
                        "old": old_name,
                        "new": new_name,
                        "torrents": torrents,
                        "cat_key": pkey,
                    }));
                }
                None => {
                    changes.push(json!({
                        "key": "category_new",
                        "new": new_name,
                        "cat_key": pkey,
                    }));
                }
                _ => {}
            }
        }
        // 失活：现存且在用，但包没声明（key 载荷按 key 对、无 key 载荷按 id 对）
        for (id, k, name, torrents) in &cat_rows {
            let declared = k.as_deref().is_some_and(|kv| declared_keys.contains(kv))
                || (k.is_none()
                    && declared_ids.contains(&(*id as i64)));
            if !declared && *torrents > 0 {
                changes.push(json!({
                    "key": "category_inactive",
                    "old": name,
                    "torrents": torrents,
                    "cat_key": k,
                }));
            }
        }
    }
    Ok((cur.len().saturating_sub(changes.len()), changes))
}

/// 分类树载荷（G20 另存 / G21 快照共用）：带 key/层级/排序/图标/分类色，且
/// **父先于子**——parent_id 外键在 INSERT 当场校验，顺序错会让子分类静默落空。
pub(super) async fn collect_categories(
    db: &PgPool,
) -> DomainResult<Vec<Value>> {
    type Row = (i32, String, String, Option<i32>, i32, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "WITH RECURSIVE d AS (SELECT id, 1 AS depth FROM categories \
         WHERE parent_id IS NULL UNION ALL SELECT c.id, d.depth + 1 FROM \
         categories c JOIN d ON c.parent_id = d.id) \
         SELECT c.id, c.name, c.icon_key, c.parent_id, c.sort, c.bg_color \
         FROM categories c LEFT JOIN d ON d.id = c.id \
         ORDER BY COALESCE(d.depth, 99), c.sort, c.id",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // key（0317）单独查一次：老库迁移中途（表已有列）与 query_as 元组解耦，
    // 避免六元组形状变更波及快照/另存两条链路的既有契约。
    // newznab_id（0325）同理——另存要带上对外分类号，回滚重放才能还原
    let keys: std::collections::HashMap<i32, String> =
        sqlx::query_as("SELECT id, key FROM categories WHERE key IS NOT NULL")
            .fetch_all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
    let nzs: std::collections::HashMap<i32, i32> =
        sqlx::query_as(
            "SELECT id, newznab_id FROM categories WHERE newznab_id IS NOT NULL",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    Ok(rows
        .into_iter()
        .map(|(id, name, icon, parent, sort, bg)| {
            json!({
                "id": id,
                "key": keys.get(&id),
                "name": name,
                "icon_key": icon,
                "parent_id": parent,
                "sort": sort,
                "bg_color": bg,
                "newznab_id": nzs.get(&id),
            })
        })
        .collect())
}

/// apply 前全量状态快照（G21 回滚基线）。回滚口径 = 把这份快照当一次 apply 执行
/// （0167 内容包先例），因此形状与包载荷对齐：site/modules/categories 是包载荷
/// 之外的站点状态，其余段直接取 collect() 的六段。
pub(super) async fn collect_state(db: &PgPool) -> DomainResult<Value> {
    let snap = collect(db).await?;
    let cats = collect_categories(db).await?;
    let settings: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN ('site_type', \
         'site_name', 'site_tagline', 'subtitle_kind') OR name LIKE \
         'module\\_%'",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get = |k: &str| -> Option<String> {
        settings
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    let modules: Map<String, Value> = settings
        .iter()
        .filter_map(|(n, v)| {
            n.strip_prefix("module_")
                .map(|k| (k.to_string(), json!(v == "yes")))
        })
        .collect();
    Ok(json!({
        "version": 1,
        "site": {
            "site_type": get("site_type"),
            "site_name": get("site_name"),
            "site_tagline": get("site_tagline"),
            "subtitle_kind": get("subtitle_kind"),
        },
        "modules": modules,
        "categories": cats,
        "sections": snap.sections,
        "tags": snap.tags,
        "classes": snap.classes,
        "economy": snap.economy,
        "metadata": snap.metadata,
        "terms": snap.terms,
    }))
}

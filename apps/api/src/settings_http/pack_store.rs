//! 内容包落库原语（M1）：快照 / 防悬挂 / 纯覆盖落库 / 登记与审计。
//! 从 pack_import.rs 按域拆出（300 行门禁）。HTTP 编排在 pack_import.rs。
//!
//! 语义口径（策划案 §3 + A2/A3）：
//! - 分类学 = **整站重建**：包外分类删除、包外未被引用维度删除；防悬挂守卫
//!   （referenced_categories）在导入/回滚两侧先行，能走到落库即无悬挂风险；
//! - 维度选项：包内维度整维重建；有种子引用包外选项的维度退化为追加对齐
//!   （宁残留不破坏，站型 apply 同口径）；
//! - theme = 白名单键 + settings_http 同一 validate_field 引擎。

use serde_json::Value;

use crate::errors::{DomainError, DomainResult};

use super::engine::validate_field;
use super::meta::MetaRow;
use super::pack_format::{TaxonomyData, THEME_KEYS};
pub(crate) use super::pack_kinds::{kind_to_json, KindRow};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 事务内拍当前分类学快照（回滚源）。dict 带 sort；空维度也落键（{}）：
/// 回滚按「包内维度整维重建」口径执行，缺键 = 不被回滚触碰。
/// B2 起维度带六类型元数据（field_type 等）——快照必须拍全，否则回放包
/// 漏字段，apply 侧会把它降级回 select（丢类型/必填/停用状态）。
pub(super) async fn snapshot_taxonomy(
    tx: &mut sqlx::PgTransaction<'_>,
) -> DomainResult<Value> {
    let cats: Vec<(i32, String, String)> =
        sqlx::query_as("SELECT id, name, icon_key FROM categories ORDER BY id")
            .fetch_all(&mut **tx)
            .await
            .map_err(internal)?;
    let kinds: Vec<KindRow> = sqlx::query_as(
        "SELECT kind, label, sort, field_type, required, multiple, enabled, \
         icon_key, bg_color FROM section_kinds ORDER BY sort, kind",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    let dict: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT kind, name, sort FROM section_dict ORDER BY kind, sort, id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    let mut dict_map: serde_json::Map<String, Value> = serde_json::Map::new();
    for (kind, nm, sort) in &dict {
        dict_map
            .entry(kind.clone())
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .expect("init as array")
            .push(serde_json::json!({ "name": nm, "sort": sort }));
    }
    let mut sections = serde_json::Map::new();
    sections.insert(
        "kinds".into(),
        Value::Array(kinds.iter().map(kind_to_json).collect()),
    );
    sections.insert("dict".into(), Value::Object(dict_map));
    Ok(serde_json::json!({
        "categories": cats.iter().map(|(id, nm, icon)| serde_json::json!({
            "id": id, "name": nm, "icon_key": icon,
        })).collect::<Vec<_>>(),
        "sections": Value::Object(sections),
    }))
}

/// theme 快照：**只拍包声明的键**，且把「导入前没有这一行」如实记成 null。
/// 旧写法把十四个白名单键全拍进快照，回滚时连包没碰过的键一起重放——
/// 既会吃掉站长在别处改过的外观，又会因重新校验而把自己锁死（见
/// `apply_theme_restore`）。
pub(super) async fn snapshot_theme(
    tx: &mut sqlx::PgTransaction<'_>,
    names: &[String],
) -> DomainResult<Value> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name = ANY($1)",
    )
    .bind(names)
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    let mut settings = serde_json::Map::new();
    for name in names {
        let cur = rows.iter().find(|(k, _)| k == name).map(|(_, v)| v);
        settings.insert(
            name.clone(),
            match cur {
                Some(v) => Value::String(v.clone()),
                None => Value::Null,
            },
        );
    }
    Ok(serde_json::json!({ "settings": Value::Object(settings) }))
}

/// 快照 → 回放包 payload（回滚路径复用）。dict 拍平成 [name]。
pub(super) fn snapshot_to_payload(snapshot: &Value) -> Value {
    let mut dict_out: serde_json::Map<String, Value> = serde_json::Map::new();
    if let Some(map) = snapshot
        .pointer("/sections/dict")
        .and_then(Value::as_object)
    {
        for (kind, items) in map {
            let names: Vec<Value> = items
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|it: &Value| {
                            it.get("name")
                                .and_then(Value::as_str)
                                .map(|s: &str| Value::String(s.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            dict_out.insert(kind.clone(), Value::Array(names));
        }
    }
    let mut sections = serde_json::Map::new();
    sections.insert(
        "kinds".into(),
        snapshot
            .pointer("/sections/kinds")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![])),
    );
    sections.insert("dict".into(), Value::Object(dict_out));
    let mut payload = serde_json::Map::new();
    payload.insert(
        "categories".into(),
        snapshot
            .get("categories")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![])),
    );
    payload.insert("sections".into(), Value::Object(sections));
    Value::Object(payload)
}

/// 防悬挂（两层，宁拒绝不悬挂，站型 apply 同口径）：
/// 1) 包内改名的分类若被种子引用 → 拒绝；
/// 2) 包未覆盖的现存分类若被种子引用 → 同样拒绝（整站重建语义要求包覆盖
///    全部在用分类，否则删除旧分类时会悬挂引用）。
pub(super) async fn referenced_categories(
    tx: &mut sqlx::PgTransaction<'_>,
    cats: &[(i32, String, String)],
) -> DomainResult<Vec<String>> {
    let mut conflicts = Vec::new();
    let covered: std::collections::HashSet<i32> =
        cats.iter().map(|(id, _, _)| *id).collect();
    let orphaned: Vec<(i32, String, i64)> = sqlx::query_as(
        "SELECT c.id, c.name, count(t.*) FROM categories c \
         JOIN torrents t ON t.category_id = c.id \
         WHERE NOT (c.id = ANY($1)) GROUP BY c.id, c.name",
    )
    .bind(covered.iter().collect::<Vec<_>>())
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    for (id, name, used) in orphaned {
        conflicts.push(format!(
            "分类 {id}「{name}」未被包覆盖但有 {used} 个种子引用（包需覆盖全部在用分类，或先转移种子）"
        ));
    }
    for (id, name, _) in cats {
        let cur: Option<(String,)> =
            sqlx::query_as("SELECT name FROM categories WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(internal)?;
        let Some((cur_name,)) = cur else { continue };
        if cur_name == *name {
            continue; // 原名保留
        }
        let used: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrents WHERE category_id = $1",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .unwrap_or(0);
        if used > 0 {
            conflicts.push(format!(
                "分类 {id}「{cur_name}」→「{name}」被 {used} 个种子引用"
            ));
        }
    }
    Ok(conflicts)
}

/// 分类学纯覆盖落库（事务内，整站重建口径）
pub(super) async fn apply_taxonomy(
    tx: &mut sqlx::PgTransaction<'_>,
    data: &TaxonomyData,
) -> DomainResult<usize> {
    // 包外分类删除（守卫已确保无悬挂风险；回滚路径同理）
    let covered: Vec<i32> = data.cats.iter().map(|(id, _, _)| *id).collect();
    sqlx::query("DELETE FROM categories WHERE NOT (id = ANY($1))")
        .bind(&covered)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    for (id, name, icon) in &data.cats {
        // 图标语义（二审 G7e 修复）：包未声明图标（空串）时保留现有图标，
        // 不抹空——与站型 apply 路径（只 UPDATE name）口径一致。
        sqlx::query(
            "INSERT INTO categories (id, name, icon_key) VALUES ($1, $2, $3) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, \
             icon_key = COALESCE(NULLIF(EXCLUDED.icon_key, ''), \
               categories.icon_key)",
        )
        .bind(id)
        .bind(name)
        .bind(icon)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    for k in &data.kinds {
        // 六类型元数据全字段落库（B3：导出/快照回放不带 = 静默降级回 select）。
        // field_type 仅 INSERT 时写入——更新侧不可改类型（存量值按旧类型解释，
        // 与后台 section-kinds PUT 同一纪律）；required/enabled 等可更新。
        sqlx::query(
            "INSERT INTO section_kinds \
               (kind, label, sort, field_type, required, multiple, enabled, \
                icon_key, bg_color) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (kind) DO UPDATE SET label = EXCLUDED.label, \
               sort = EXCLUDED.sort, required = EXCLUDED.required, \
               multiple = EXCLUDED.multiple, enabled = EXCLUDED.enabled, \
               icon_key = EXCLUDED.icon_key, bg_color = EXCLUDED.bg_color",
        )
        .bind(&k.kind)
        .bind(&k.label)
        .bind(k.sort)
        .bind(&k.field_type)
        .bind(k.required)
        .bind(k.multiple)
        .bind(k.enabled)
        .bind(&k.icon_key)
        .bind(&k.bg_color)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    apply_taxonomy_sections(tx, data).await?;
    Ok(data.cats.len())
}

/// theme 落库：逐键白名单 + validate_field（settings_http 同一校验引擎）。
/// 返回 (将变更键, 校验错误)。
pub(super) async fn apply_theme_import(
    tx: &mut sqlx::PgTransaction<'_>,
    settings: &serde_json::Map<String, Value>,
    by_name: &std::collections::HashMap<String, MetaRow>,
) -> DomainResult<(Vec<String>, Vec<(String, String)>)> {
    let mut errors = Vec::new();
    let mut changed = Vec::new();
    for (name, raw) in settings {
        if !THEME_KEYS.contains(&name.as_str()) {
            errors.push((name.clone(), "theme 包不允许触碰该设置键".into()));
            continue;
        }
        let Some(meta) = by_name.get(name) else {
            errors.push((name.clone(), "设置键不存在".into()));
            continue;
        };
        let raw = raw.as_str().unwrap_or_default();
        match validate_field(meta, raw) {
            Ok(normalized) => {
                if normalized != meta.value {
                    sqlx::query(
                        "INSERT INTO site_settings (name, value) \
                         VALUES ($1, $2) ON CONFLICT (name) DO UPDATE \
                         SET value = EXCLUDED.value, updated_at = now()",
                    )
                    .bind(name)
                    .bind(&normalized)
                    .execute(&mut **tx)
                    .await
                    .map_err(internal)?;
                    changed.push(name.clone());
                }
            }
            Err(e) => errors.push((name.clone(), e)),
        }
    }
    Ok((changed, errors))
}

/// theme 回滚：逐键写回快照原值，**不重新校验**。
/// 批四十实测立案依据：快照是导入前从库里拍下来的现值，回滚再校验一遍等于
/// 「装包时合法、回滚时被自己的校验挡在门外」——八个品牌色键的现值是空串，
/// `color` 校验直接报「须为合法 hex 颜色」，任何没填齐八色的站点都装得上
/// theme 包、却永远滚不回去（登记行也删不掉，400 在删行之前）。
/// 快照里的 null = 导入前没有这一行 → 删掉，回到未设置=默认（A2 还原语义）。
pub(super) async fn apply_theme_restore(
    tx: &mut sqlx::PgTransaction<'_>,
    settings: &serde_json::Map<String, Value>,
) -> DomainResult<(Vec<String>, Vec<(String, String)>)> {
    let mut changed = Vec::new();
    let mut errors = Vec::new();
    for (name, raw) in settings {
        if !THEME_KEYS.contains(&name.as_str()) {
            errors.push((name.clone(), "theme 包不允许触碰该设置键".into()));
            continue;
        }
        if raw.is_null() {
            sqlx::query("DELETE FROM site_settings WHERE name = $1")
                .bind(name)
                .execute(&mut **tx)
                .await
                .map_err(internal)?;
            changed.push(name.clone());
            continue;
        }
        sqlx::query(
            "INSERT INTO site_settings (name, value) VALUES ($1, $2) \
             ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
             updated_at = now()",
        )
        .bind(name)
        .bind(raw.as_str().unwrap_or_default())
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
        changed.push(name.clone());
    }
    Ok((changed, errors))
}

/// 回滚侧的分类学落库：守卫（快照分类改名也不得悬挂）+ 纯覆盖
pub(super) async fn apply_taxonomy_guarded(
    tx: &mut sqlx::PgTransaction<'_>,
    data: &TaxonomyData,
) -> DomainResult<()> {
    let conflicts = referenced_categories(tx, &data.cats).await?;
    if !conflicts.is_empty() {
        return Err(DomainError::Validation(format!(
            "回滚会悬挂种子引用，已拒绝：{}",
            conflicts.join("；")
        )));
    }
    apply_taxonomy(tx, data).await?;
    Ok(())
}

async fn apply_taxonomy_sections(
    tx: &mut sqlx::PgTransaction<'_>,
    data: &TaxonomyData,
) -> DomainResult<()> {
    let mut kinds_in_pack: std::collections::HashSet<String> =
        data.kinds.iter().map(|k| k.kind.clone()).collect();
    for (k, _) in &data.dict {
        kinds_in_pack.insert(k.clone());
    }
    // 包外维度清理（整站重建口径；被引用的跳过——宁残留不破坏）
    let existing_kinds: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM section_kinds")
            .fetch_all(&mut **tx)
            .await
            .map_err(internal)?;
    for kind in &existing_kinds {
        if kinds_in_pack.contains(kind) {
            continue;
        }
        let in_use: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrent_sections ts \
             JOIN section_dict sd ON sd.id = ts.dict_id \
             WHERE sd.kind = $1",
        )
        .bind(kind)
        .fetch_one(&mut **tx)
        .await
        .map_err(internal)?;
        if in_use == 0 {
            sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
                .bind(kind)
                .execute(&mut **tx)
                .await
                .map_err(internal)?;
        }
    }
    for kind in &kinds_in_pack {
        // dict-only 维度确保维度行存在（field_type 缺省 select——字典项维度
        // 本就是枚举形态，与 B2 之前的历史口径一致）
        sqlx::query(
            "INSERT INTO section_kinds (kind, label, sort, field_type) \
             VALUES ($1, $1, 999, 'select') ON CONFLICT (kind) DO NOTHING",
        )
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
        let packed: Vec<String> = data
            .dict
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, n)| n.clone())
            .collect();
        // 该维度只要有任一条选项被种子引用，就不能整维重建：重建 = 先
        // DELETE 全维再按包重插（新 id），而 `torrent_sections.dict_id` 是
        // ON DELETE CASCADE，包内同名选项照删不误 —— 种子的维度标注跟着
        // 旧 id 一起蒸发（批四十 P-144：站型分类学包一装，在用标注清零）。
        // 退化为「只追加、不动在用行」：id 稳定，包外选项残留（A3 口径）。
        let in_use: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrent_sections ts \
             JOIN section_dict sd ON sd.id = ts.dict_id \
             WHERE sd.kind = $1",
        )
        .bind(kind)
        .fetch_one(&mut **tx)
        .await
        .map_err(internal)?;
        if in_use > 0 {
            // 退化口径：只追加缺失选项，不动在用选项
            for name in &packed {
                sqlx::query(
                    "INSERT INTO section_dict (kind, name, sort) \
                     SELECT $1, $2, 999 WHERE NOT EXISTS (\
                       SELECT 1 FROM section_dict WHERE kind = $1 \
                       AND name = $2)",
                )
                .bind(kind)
                .bind(name)
                .execute(&mut **tx)
                .await
                .map_err(internal)?;
            }
            continue;
        }
        sqlx::query("DELETE FROM section_dict WHERE kind = $1")
            .bind(kind)
            .execute(&mut **tx)
            .await
            .map_err(internal)?;
        for (i, name) in packed.iter().enumerate() {
            sqlx::query(
                "INSERT INTO section_dict (kind, name, sort) \
                 VALUES ($1, $2, $3)",
            )
            .bind(kind)
            .bind(name)
            .bind((i + 1) as i32)
            .execute(&mut **tx)
            .await
            .map_err(internal)?;
        }
    }
    Ok(())
}

// 素材包（kind=assets）的快照与落库已按域拆到 pack_assets.rs：那两张表的
// 删除范围要跟着「包声明了哪张表」走，和这里的分类学/theme 原语不是同一
// 套判据。

//! 内容包的格式解析与导出（M1/M3）。从 packs.rs 按域拆出（300 行门禁）。
//! 落库与回滚在 pack_import.rs。
//!
//! 包 JSON 顶层：
//! { "format": "fluxtorrent.contentpack", "version": 1,
//!   "kind": "taxonomy" | "theme" | "rules",
//!   "pack_id": "taxonomy.movie.v1", "name": "影视分类学", "core_compat": ">=1.4",
//!   "payload": { ... 按 kind 解释 ... } }
//!
//! taxonomy payload：{ categories: [{id,name,icon_key}], sections: {kinds:[...],
//! dict:{kind:[names]}} }；theme payload：{ settings: {外观键: 值} }（键走
//! settings_meta 白名单，外观组以外一律拒绝——theme 包碰不了其它设定）；
//! rules payload：{ rules: {rule_键: 表达式} }（键走规则白名单 + 表达式过
//! rules_engine lint——rules 包碰不了规则以外的任何设定）。

use actix_web::{web, HttpResponse};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::meta::{MetaRow, META_SELECT};

pub(super) const FORMAT: &str = "fluxtorrent.contentpack";
/// theme 包允许触碰的 settings 键（外观白名单口径）
pub(super) const THEME_KEYS: &[&str] = &[
    "site_name",
    "site_tagline",
    "site_logo",
    "site_desc",
    "logo",
    "currency_name",
];
/// rules 包允许触碰的规则键 → 求值 RuleSpec（M3 首批：银行利率两键）
pub(super) fn rule_spec_for(key: &str) -> Option<crate::rules_engine::RuleSpec>
{
    match key {
        "rule_bank_term_rate" => Some(crate::rules_engine::RuleSpec {
            key: "bank.term_rate",
            vars: &["term_days"],
            fallback: 0.0,
            min: 0.0,
            max: 1.0,
        }),
        "rule_bank_loan_daily" => Some(crate::rules_engine::RuleSpec {
            key: "bank.loan_daily_rate",
            vars: &["term_days"],
            fallback: 0.0,
            min: 0.0,
            max: 0.01,
        }),
        _ => None,
    }
}

pub(super) fn bad(msg: &str) -> DomainError {
    DomainError::Validation(msg.to_string())
}

/// 解析后的包（payload 保留原样，按 kind 在落库侧二次解释）
pub(super) struct PackHead {
    pub kind: String,
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub core_compat: String,
    pub payload: Value,
}

pub(super) fn parse_pack(pack: &Value) -> DomainResult<PackHead> {
    let obj = pack.as_object().ok_or_else(|| bad("包必须是 JSON 对象"))?;
    if obj.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(bad("format 不识别（需 fluxtorrent.contentpack）"));
    }
    if obj.get("version").and_then(Value::as_i64) != Some(1) {
        return Err(bad("version 不支持（当前仅 1）"));
    }
    let kind = obj
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !["taxonomy", "theme", "rules"].contains(&kind.as_str()) {
        return Err(bad("kind 需为 taxonomy / theme / rules"));
    }
    let pack_id = obj
        .get("pack_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if pack_id.is_empty() || pack_id.len() > 100 {
        return Err(bad("pack_id 必填且 ≤100 字符"));
    }
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&pack_id)
        .to_string();
    let version = obj
        .get("pack_version")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "1.0.0".into());
    let core_compat = obj
        .get("core_compat")
        .and_then(Value::as_str)
        .unwrap_or("*")
        .to_string();
    let payload = obj
        .get("payload")
        .cloned()
        .ok_or_else(|| bad("缺少 payload 节"))?;
    Ok(PackHead { kind, pack_id, name, version, core_compat, payload })
}

// ============ 导出：当前站点 → 包文件 ============

pub(super) async fn export_taxonomy(
    state: &web::Data<std::sync::Arc<AppState>>,
    name: Option<&str>,
) -> DomainResult<HttpResponse> {
    let cats: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT id, name, icon_key FROM categories ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let kinds: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT kind, label, sort FROM section_kinds ORDER BY sort, kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let dict: Vec<(String, String)> = sqlx::query_as(
        "SELECT kind, name FROM section_dict ORDER BY kind, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut dict_map: serde_json::Map<String, Value> = serde_json::Map::new();
    for (kind, nm) in &dict {
        dict_map
            .entry(kind.clone())
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .expect("init as array")
            .push(Value::String(nm.clone()));
    }
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "custom".into());
    Ok(ok(serde_json::json!({
        "format": FORMAT,
        "version": 1,
        "kind": "taxonomy",
        "pack_id": format!("taxonomy.{site_type}.export"),
        "name": name.unwrap_or("当前站点分类学快照"),
        "core_compat": "*",
        "exported_at": chrono::Utc::now(),
        "payload": {
            "categories": cats.iter().map(|(id, nm, icon)| serde_json::json!({
                "id": id, "name": nm, "icon_key": icon,
            })).collect::<Vec<_>>(),
            "sections": {
                "kinds": kinds.iter().map(|(k, label, sort)| serde_json::json!({
                    "kind": k, "label": label, "sort": sort,
                })).collect::<Vec<_>>(),
                "dict": Value::Object(dict_map),
            },
        },
    })))
}

pub(super) async fn export_theme(
    state: &web::Data<std::sync::Arc<AppState>>,
    name: Option<&str>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = ANY($1)"))
            .bind(THEME_KEYS)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let mut settings = serde_json::Map::new();
    let mut masked: Vec<String> = Vec::new();
    for m in &rows {
        if m.secret {
            settings.insert(m.name.clone(), Value::String(String::new()));
            masked.push(m.name.clone());
        } else {
            settings.insert(m.name.clone(), Value::String(m.value.clone()));
        }
    }
    Ok(ok(serde_json::json!({
        "format": FORMAT,
        "version": 1,
        "kind": "theme",
        "pack_id": "theme.current.export",
        "name": name.unwrap_or("当前站点外观快照"),
        "core_compat": "*",
        "exported_at": chrono::Utc::now(),
        "masked_secrets": masked,
        "payload": { "settings": Value::Object(settings) },
    })))
}

// ============ taxonomy payload 解析 ============

pub(super) struct TaxonomyData {
    pub cats: Vec<(i32, String, String)>,
    pub kinds: Vec<(String, String, i32)>,
    pub dict: Vec<(String, String)>,
}

pub(super) fn parse_taxonomy(payload: &Value) -> DomainResult<TaxonomyData> {
    let mut cats = Vec::new();
    if let Some(arr) = payload.get("categories").and_then(Value::as_array) {
        for (i, c) in arr.iter().enumerate() {
            let id = c
                .get("id")
                .and_then(Value::as_i64)
                .unwrap_or(i as i64 + 1) as i32;
            let name = c
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let icon = c
                .get("icon_key")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            cats.push((id, name, icon));
        }
    }
    if cats.is_empty() {
        return Err(bad("taxonomy 包至少需要 1 个有效分类"));
    }
    let mut kinds = Vec::new();
    if let Some(arr) =
        payload.pointer("/sections/kinds").and_then(Value::as_array)
    {
        for k in arr {
            let (Some(kind), Some(label)) = (
                k.get("kind").and_then(Value::as_str),
                k.get("label").and_then(Value::as_str),
            ) else {
                continue;
            };
            if !kind
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(bad(&format!(
                    "维度 kind 需为 ASCII 标识：{kind}"
                )));
            }
            let sort =
                k.get("sort").and_then(Value::as_i64).unwrap_or(999) as i32;
            kinds.push((kind.to_string(), label.to_string(), sort));
        }
    }
    let mut dict = Vec::new();
    if let Some(map) =
        payload.pointer("/sections/dict").and_then(Value::as_object)
    {
        for (kind, names) in map {
            for n in names.as_array().cloned().unwrap_or_default() {
                if let Some(nm) = n.as_str() {
                    dict.push((kind.clone(), nm.to_string()));
                }
            }
        }
    }
    Ok(TaxonomyData { cats, kinds, dict })
}

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
use super::pack_kinds::{kind_from_json, kind_to_json, KindRow};

pub(super) const FORMAT: &str = "fluxtorrent.contentpack";
/// theme 包允许触碰的 settings 键（外观白名单口径）
pub(super) const THEME_KEYS: &[&str] = &[
    "site_name",
    "site_tagline",
    "site_logo",
    "site_desc",
    "logo",
    "currency_name",
    // 主题令牌（0189 R4.6）：品牌八色随 theme 包分发
    "theme_token_sky",
    "theme_token_sun",
    "theme_token_coral",
    "theme_token_mint",
    "theme_token_candy",
    "theme_token_indigo",
    "theme_token_glow",
    "theme_token_ribbon",
];
/// rules 包允许触碰的规则键 → 求值 RuleSpec（M3 首批：银行利率两键）
pub(super) fn rule_spec_for(
    key: &str,
) -> Option<crate::rules_engine::RuleSpec> {
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
    if !["taxonomy", "theme", "rules", "assets", "embed"]
        .contains(&kind.as_str())
    {
        return Err(bad("kind 需为 taxonomy / theme / rules / assets / embed"));
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
    // R11：core_compat 接入实际校验——仅支持 "*"/">=" + 版本号格式，
    // 不识别的表达式拒收（此前只存不校验，是死字段）
    {
        let compat = core_compat.trim();
        let compat_ok = compat == "*"
            || (compat.starts_with(">=")
                && !compat[2..].is_empty()
                && compat[2..].chars().all(|c| c.is_ascii_digit() || c == '.'));
        if !compat_ok {
            return Err(bad(
                "core_compat 格式不支持（仅 * 或 >=版本号，如 >=1.4）",
            ));
        }
    }
    let payload = obj
        .get("payload")
        .cloned()
        .ok_or_else(|| bad("缺少 payload 节"))?;
    Ok(PackHead {
        kind,
        pack_id,
        name,
        version,
        core_compat,
        payload,
    })
}

// ============ 导出：当前站点 → 包文件 ============

pub(super) async fn export_taxonomy(
    state: &web::Data<std::sync::Arc<AppState>>,
    name: Option<&str>,
) -> DomainResult<HttpResponse> {
    let cats: Vec<(i32, String, String)> =
        sqlx::query_as("SELECT id, name, icon_key FROM categories ORDER BY id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let kinds: Vec<KindRow> = sqlx::query_as(
        "SELECT kind, label, sort, field_type, required, multiple, enabled, \
         icon_key, bg_color FROM section_kinds ORDER BY sort, kind",
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
                "kinds": kinds.iter().map(kind_to_json).collect::<Vec<_>>(),
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
    pub kinds: Vec<KindRow>,
    pub dict: Vec<(String, String)>,
}

pub(super) fn parse_taxonomy(payload: &Value) -> DomainResult<TaxonomyData> {
    let mut cats = Vec::new();
    if let Some(arr) = payload.get("categories").and_then(Value::as_array) {
        for (i, c) in arr.iter().enumerate() {
            let id = c.get("id").and_then(Value::as_i64).unwrap_or(i as i64 + 1)
                as i32;
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
            let Some(kind) = k.get("kind").and_then(Value::as_str) else {
                continue;
            };
            if k.get("label").and_then(Value::as_str).is_none() {
                continue;
            }
            if !kind.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(bad(&format!("维度 kind 需为 ASCII 标识：{kind}")));
            }
            kinds.push(kind_from_json(k, &bad)?);
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

// ============ 素材包（M1 增量，kind=assets） ============

/// 素材表白名单：表名 →（列清单, 软上限）。越权表在 parse 阶段即拒。
pub(super) const ASSET_TABLES: &[&str] = &["medals", "avatar_frames"];

/// 素材包导出：两张表全量 → 包文件（行原样保留未知列以外的已知列）
pub(super) async fn export_assets(
    state: &web::Data<std::sync::Arc<AppState>>,
    name: Option<&str>,
) -> DomainResult<HttpResponse> {
    let medals: Vec<(
        i64,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i32>,
        i16,
        Option<i32>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        f64,
        i32,
        i32,
    )> = sqlx::query_as(
        "SELECT id, name, price, rarity, description, asset_ref, \
         duration_days, get_type, inventory, sale_begin_at, sale_end_at, \
         bonus_addition_factor::float8, category_id, limited::int \
         FROM medals ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    let frames: Vec<(i32, String, String, i32, i32, Option<String>)> =
        sqlx::query_as(
            "SELECT id, name, css, price, sort, image_url \
             FROM avatar_frames ORDER BY id",
        )
        .fetch_all(&state.repo.db)
        .await
        .map_err(internal)?;
    let medals_json: Vec<Value> = medals
        .into_iter()
        .map(|m| {
            let (
                id,
                name,
                price,
                rarity,
                description,
                asset_ref,
                duration_days,
                get_type,
                inventory,
                sale_begin_at,
                sale_end_at,
                bonus,
                category_id,
                limited,
            ) = m;
            serde_json::json!({
                "id": id, "name": name, "price": price, "rarity": rarity,
                "description": description, "asset_ref": asset_ref,
                "duration_days": duration_days, "get_type": get_type,
                "inventory": inventory, "sale_begin_at": sale_begin_at,
                "sale_end_at": sale_end_at,
                "bonus_addition_factor": bonus, "category_id": category_id,
                "limited": limited != 0,
            })
        })
        .collect();
    let frames_json: Vec<Value> = frames
        .into_iter()
        .map(|(id, name, css, price, sort, image_url)| {
            serde_json::json!({
                "id": id, "name": name, "css": css, "price": price,
                "sort": sort, "image_url": image_url,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "format": FORMAT,
        "version": 1,
        "kind": "assets",
        "pack_id": "assets.current.export",
        "name": name.unwrap_or("当前站点素材快照"),
        "core_compat": "*",
        "exported_at": chrono::Utc::now(),
        "payload": {
            "tables": {
                "medals": medals_json,
                "avatar_frames": frames_json,
            },
        },
    })))
}

fn internal<E: Into<anyhow::Error>>(e: E) -> DomainError {
    DomainError::Internal(e.into())
}

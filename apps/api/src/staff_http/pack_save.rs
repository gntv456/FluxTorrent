//! 站型包 diff 与自定义保存。
//! 从 staff_http.rs 按域拆出。

use super::sitetype::SiteTypePack;
use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[post("/admin/site-type-packs/diff")]
pub async fn site_type_pack_diff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackDiffBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, \
         sort FROM site_type_packs WHERE code = $1",
    )
    .bind(&body.code)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };
    // 当前值快照（site_type/site_name/module_*）
    let current: Vec<(String, String)> =
                sqlx::query_as("SELECT name, \
         value FROM site_settings WHERE name IN ('site_type','site_name') OR name LIKE 'module\\_%'")
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    let cur = std::collections::HashMap::<String, String>::from_iter(current);
    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut push = |key: &str, old: Option<&String>, new: &str| {
        let old_v = old.map(|s| s.as_str()).unwrap_or("(未设置)");
        if old_v != new {
            changes.push(
                serde_json::json!({ "key": key, "old": old_v, "new": new }),
            );
        }
    };
    push("site_type", cur.get("site_type"), &pack.code);
    push("site_name", cur.get("site_name"), &pack.brand);
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
    Ok(ok(serde_json::json!({
        "pack": pack.code,
        "name": pack.name,
        "changes": changes,
        "unchanged_modules": cur.len().saturating_sub(changes.len()),
    })))
}

/// 自定义站型另存（U2 §7.3 / U5 分发）：读当前站点配置快照存为新包
/// （code 前缀 custom_），预置包只读——满足「第 12 种站型」。
#[derive(Deserialize)]
struct PackSaveBody {
    code: String,
    name: String,
}
#[post("/admin/site-type-packs/save")]
pub async fn site_type_pack_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackSaveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let code = body.code.trim().to_lowercase();
    if !code.starts_with("custom_")
        || code.len() > 40
        || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(DomainError::Validation(
            "code 需以 custom_ 开头，仅小写字母/数字/下划线，≤40 字符".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("name 不能为空".into()));
    }
    // 当前配置快照：分类 / 模块开关 / 站名
    let cats: Vec<serde_json::Value> = sqlx::query_as::<_, (i32, String)>(
        "SELECT id, name FROM categories ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
    .collect();
    let mods_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name LIKE 'module\\_%'",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let modules: serde_json::Map<String, serde_json::Value> = mods_rows
        .into_iter()
        .filter_map(|(name, value)| {
            name.strip_prefix("module_")
                .map(|k| (k.to_string(), serde_json::json!(value == "yes")))
        })
        .collect();
    let brand: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();
    // 自定义包的默认标语 = 快照时刻的登录页标语（覆盖值优先，空回落当前站型包默认）
    let pack_default_tagline: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT p.tagline FROM site_type_packs p \
         JOIN site_settings s ON s.name = 'site_type' AND s.value = p.code",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|t| !t.trim().is_empty());
    let override_tagline: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_tagline'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    let tagline = override_tagline
        .or(pack_default_tagline)
        .unwrap_or_default();
    // 字幕口径快照（0178）：custom 包带上当前 kind，apply 时按快照显式应用
    // （二审 G7d：music/lossless 站另存再 apply 不再把歌词口径重置回影视字幕）
    let subtitle_kind: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'subtitle_kind'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v| v == "lyric" || v == "subtitle");
    let sort: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(sort), 100) + 1 FROM site_type_packs",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(101);
    sqlx::query(
        "INSERT INTO site_type_packs (code, name, description, brand, categories, modules, sort, tagline, subtitle_kind) \
         VALUES ($1, $2, '自定义站型（另存快照）', $3, $4::jsonb, $5::jsonb, $6, $7, $8) \
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name, brand = EXCLUDED.brand, \
           categories = EXCLUDED.categories, modules = EXCLUDED.modules, tagline = EXCLUDED.tagline, \
           subtitle_kind = EXCLUDED.subtitle_kind",
    )
    .bind(&code)
    .bind(body.name.trim())
    .bind(&brand)
    .bind(serde_json::Value::Array(cats).to_string())
    .bind(serde_json::Value::Object(modules).to_string())
    .bind(sort)
    .bind(&tagline)
    .bind(subtitle_kind)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_save", None)
        .await;
    Ok(ok(serde_json::json!({ "saved": code })))
}

// ---- 捐赠中心（馒头 donate 口径：储值钱包 + 三区套餐 + VIP）----

/// 站型切换 diff 预览（U2 §8.2 向导第二步）：返回 apply 将改动的键旧值→新值，
/// 不落库。站长确认后才走 apply。
#[derive(Deserialize)]
pub(super) struct PackDiffBody {
    pub(super) code: String,
}

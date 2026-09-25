//! 内容包（生态商店 M1，策划案 §3）：taxonomy / theme 两类包的导出、导入、回滚。
//! 挂在 settings_http 域下（后台「导入导出」页的同族能力），按 300 行门禁拆三个文件：
//! 包格式与导出在 pack_format.rs，纯覆盖落库/快照/回滚在 pack_import.rs，HTTP 端点在此。
//!
//! M1 纪律（对应策划案不变式）：
//! - 包只做「纯覆盖 + 动态合并」，不物化派生值——分类/维度落 categories /
//!   section_kinds / section_dict，页面侧继续动态派生（0145）；
//! - 导入前必拍快照进 content_packs.snapshot，回滚 = 把快照按同一条纯覆盖路径
//!   重放（A2 禁用即还原）；
//! - 分类重建沿用站型 apply 的防悬挂口径：有种子引用的分类整表替换被拒绝，
//!   单分类改名同样被拒绝（宁拒绝不悬挂）；
//! - theme 包只写 settings_meta 已登记的外观键（复用 validate_field），未知键拒绝。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::pack_format;
use super::pack_import;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 导出（把当前站点的分类学/外观导出为包文件） ============

#[derive(Deserialize)]
struct ExportQ {
    /// taxonomy | theme
    kind: String,
    name: Option<String>,
}

#[get("/admin/content-packs/export")]
async fn pack_export(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ExportQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    match q.kind.as_str() {
        "taxonomy" => {
            pack_format::export_taxonomy(&state, q.name.as_deref()).await
        }
        "theme" => pack_format::export_theme(&state, q.name.as_deref()).await,
        "assets" => pack_format::export_assets(&state, q.name.as_deref()).await,
        _ => Err(DomainError::Validation(
            "kind 需为 taxonomy / theme / assets".into(),
        )),
    }
}

// ============ 导入：预检（diff 空跑）与落库 ============

#[derive(Deserialize)]
struct ImportBody {
    pack: serde_json::Value,
    #[serde(default)]
    confirm: bool,
}

#[post("/admin/content-packs/import")]
async fn pack_import_ep(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ImportBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 导入与回滚同权：sysop 级（与 settings import 对齐）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    pack_import::import(&state, &auth.id, &body.pack, body.confirm).await
}

// ============ 已装包列表 + 回滚 ============

#[get("/admin/content-packs")]
async fn pack_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(
        i64,
        String,
        String,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, pack_id, kind, name, version, applied_at \
             FROM content_packs ORDER BY applied_at DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows
        .into_iter()
        .map(|(id, pack_id, kind, name, version, at)| {
            serde_json::json!({
                "id": id, "pack_id": pack_id, "kind": kind,
                "name": name, "version": version, "applied_at": at,
            })
        })
        .collect::<Vec<_>>()))
}

#[derive(Deserialize)]
struct RollbackBody {
    id: i64,
}

#[post("/admin/content-packs/rollback")]
async fn pack_rollback(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RollbackBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    pack_import::rollback(&state, &auth.id, body.id).await
}

#[derive(Deserialize)]
struct RuleTryBody {
    /// 规则键（白名单内）
    key: String,
    /// 表达式（空 = 清除）
    expr: String,
    /// 试算变量（如 term_days: 90）
    #[serde(default)]
    vars: std::collections::HashMap<String, f64>,
}

/// POST /admin/content-packs/rule-try —— 表达式试算（lint + 代入变量求值，
/// 不落库）。规则包编辑器的「预检」后端。
#[post("/admin/content-packs/rule-try")]
async fn rule_try(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RuleTryBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let Some(spec) = pack_format::rule_spec_for(body.key.as_str()) else {
        return Err(DomainError::Validation("未知规则键（不在白名单）".into()));
    };
    let expr = body.expr.trim();
    if !expr.is_empty() {
        crate::rules_engine::lint(expr, &spec)?;
    }
    let mut vars: std::collections::HashMap<&str, f64> =
        std::collections::HashMap::new();
    for v in spec.vars {
        vars.insert(v, body.vars.get(*v).copied().unwrap_or(0.0));
    }
    let value = if expr.is_empty() {
        None
    } else {
        Some(crate::rules_engine::eval(expr, &spec, &vars))
    };
    Ok(ok(serde_json::json!({
        "key": body.key,
        "expr": expr,
        "value": value,
        "domain": [spec.min, spec.max],
        "vars": spec.vars,
    })))
}

pub fn mount_content_packs(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(pack_export)
        .service(pack_import_ep)
        .service(pack_list)
        .service(pack_rollback)
        .service(rule_try)
        .service(super::pack_catalog::pack_catalog)
        .service(super::pack_catalog::pack_install)
}

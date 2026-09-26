//! 术语表后台 CRUD（0205 / 四审 L7）。
//!
//! 一张行表 + 三条端点：列 / 写（upsert）/ 删。写侧落库后**立刻** `terms::reload`，
//! 让改词在下一个请求就生效（不走 TTL：「改了看不到」那类死锁已踩过一次）。
//! 命中预览放在前端（`apply-terms.ts` 扫字典），因为只有 web 侧拿得到界面文案本体。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::terms;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(crate) struct TermRow {
    canonical: String,
    replacement: String,
    enabled: bool,
    descr: String,
    sort: i32,
}

#[get("/admin/terms")]
pub async fn terms_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let rows: Vec<TermRow> = sqlx::query_as(
        "SELECT canonical, replacement, enabled, descr, sort \
         FROM site_terms ORDER BY length(canonical) DESC, sort, canonical",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TermBody {
    canonical: String,
    replacement: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default = "default_sort")]
    sort: i32,
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

/// 写一条规则（canonical 存在即覆盖）。
#[post("/admin/terms")]
pub async fn terms_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TermBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let from = body.canonical.trim();
    let to = body.replacement.trim();
    if !terms::valid_term_word(from) || !terms::valid_term_word(to) {
        return Err(DomainError::Validation(
            "词条需 1-20 字符且不含花括号（{n}/{magic} 是插值占位符）".into(),
        ));
    }
    if from == to {
        return Err(DomainError::Validation(
            "规范词与替换词相同：这条规则什么都不会改".into(),
        ));
    }
    let live: i64 =
        sqlx::query_scalar("SELECT count(*) FROM site_terms WHERE enabled")
            .fetch_one(&state.repo.db)
            .await
            .map_err(internal)?;
    let known: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM site_terms WHERE canonical = $1",
    )
    .bind(from)
    .fetch_one(&state.repo.db)
    .await
    .map_err(internal)?;
    if known == 0 && body.enabled && live >= terms::MAX_RULES {
        return Err(DomainError::Validation(format!(
            "术语规则上限 {} 条（错误出口按条数线性扫描文案）",
            terms::MAX_RULES
        )));
    }
    sqlx::query(
        "INSERT INTO site_terms \
         (canonical, replacement, enabled, descr, sort) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (canonical) DO UPDATE SET \
         replacement = EXCLUDED.replacement, \
         enabled = EXCLUDED.enabled, \
         descr = EXCLUDED.descr, sort = EXCLUDED.sort, \
         updated_at = now()",
    )
    .bind(from)
    .bind(to)
    .bind(body.enabled)
    .bind(body.descr.as_deref().unwrap_or("").trim())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(internal)?;
    let n = terms::reload(&state.repo.db).await;
    Ok(ok(serde_json::json!({
        "canonical": from,
        "replacement": to,
        "enabled_rules": n,
    })))
}

/// 停用/启用一条规则（保留词条，只切 enabled）。
#[post("/admin/terms/{canonical}/toggle")]
pub async fn terms_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let hit: Option<bool> = sqlx::query_scalar(
        "UPDATE site_terms SET enabled = NOT enabled, updated_at = now() \
         WHERE canonical = $1 RETURNING enabled",
    )
    .bind(path.as_str())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let enabled =
        hit.ok_or_else(|| DomainError::Validation("没有这条术语规则".into()))?;
    terms::reload(&state.repo.db).await;
    Ok(ok(serde_json::json!({ "enabled": enabled })))
}

/// 删一条规则。confirm 字段与自定义页面/字段面板同形（危险操作要显式确认）。
#[derive(Deserialize)]
struct TermDeleteBody {
    #[serde(default)]
    confirm: bool,
}

#[post("/admin/terms/{canonical}/delete")]
pub async fn terms_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<TermDeleteBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    if !body.confirm {
        return Err(DomainError::Validation("请确认删除".into()));
    }
    let n = sqlx::query("DELETE FROM site_terms WHERE canonical = $1")
        .bind(path.as_str())
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    terms::reload(&state.repo.db).await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

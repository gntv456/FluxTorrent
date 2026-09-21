//! 友情链接管理（linksmanage.php 复刻：申请/审核/编辑/删除）。
//! 从 http.rs 按域拆出。

use super::auth_infra::require_auth;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

// ============ 友情链接管理（linksmanage.php 复刻：申请/审核/编辑/删除） ============

#[derive(Deserialize)]
struct LinkApplyBody {
    name: String,
    url: String,
    #[serde(default)]
    title: Option<String>,
    admin: String,
    email: String,
    reason: String,
}

/// 申请友链（进 pending，staff 审核后 active）
#[post("/links/apply")]
pub async fn link_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LinkApplyBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.name.trim().is_empty() || body.url.trim().is_empty() {
        return Err(DomainError::Validation("站点名和 URL 必填".into()));
    }
    if !body.email.contains('@') {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    if body.reason.trim().chars().count() < 20 {
        return Err(DomainError::Validation("申请理由至少 20 字".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO friend_links (name, url, title, status, \
         applied_by, admin_name, email, reason) VALUES ($1, $2, $3, 'pending', \
         $4, $5, $6, $7) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(&body.title)
    .bind(auth.id)
    .bind(body.admin.trim())
    .bind(body.email.trim())
    .bind(body.reason.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_apply", None).await;
    Ok(ok(serde_json::json!({ "id": id, "status": "pending" })))
}

#[derive(Deserialize)]
struct LinkAdminBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    status: Option<String>,
}

/// 编辑/审核友链（staff；status: pending→active 通过）
#[put("/admin/links/{id}")]
pub async fn link_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<LinkAdminBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE)
        .await?;
    if let Some(st) = &body.status {
        if !["pending", "active", "hidden"].contains(&st.as_str()) {
            return Err(DomainError::Validation("非法状态".into()));
        }
    }
    let updated = sqlx::query(
        "UPDATE friend_links SET \
            name = COALESCE($2, name), url = COALESCE($3, url), title = COALESCE($4, title), \
            sort = COALESCE($5, sort), status = COALESCE($6, status) \
         WHERE id = $1",
    )
    .bind(*path)
    .bind(&body.name)
    .bind(&body.url)
    .bind(&body.title)
    .bind(body.sort)
    .bind(&body.status)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "link_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/links/{id}")]
pub async fn link_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE)
        .await?;
    sqlx::query("DELETE FROM friend_links WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct LinkRow {
    id: i32,
    name: String,
    url: String,
    title: Option<String>,
    status: String,
    #[sqlx(default)]
    applied_by: Option<i64>,
    #[sqlx(default)]
    admin_name: Option<String>,
    #[sqlx(default)]
    email: Option<String>,
    #[sqlx(default)]
    reason: Option<String>,
}

/// 友链管理列表（staff，含 pending）
#[get("/admin/links")]
pub async fn link_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE)
        .await?;
    let rows: Vec<LinkRow> = sqlx::query_as(
        "SELECT id, name, url, title, status, applied_by, admin_name, email, reason \
         FROM friend_links ORDER BY (status = 'pending') DESC, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

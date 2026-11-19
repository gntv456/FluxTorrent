//! 竞赛运营补全（0503，体检七轮运营台账 #2）：
//! contests 此前是「能报名、能看榜首」的空转骨架——无建赛入口、无计分、
//! 无结算。本批三件：
//!   · admin CRUD（建/编辑/关闭赛，SETTINGS_MANAGE 权限）
//!   · 计分口径 = 上传竞速：窗口内过审种数（admin 可在 descr 写明赛制）
//!   · 结算 job（worker contest_settle）：到期赛按 prizes JSONB
//!     [{"rank":N,"sparks":M},...] 发火花 + PM，settled_at 幂等锚
//! 模块默认关（module_contests），开赛即自动获得运营位。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::meta::staff;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 竞赛管理 = STAFF_PANEL + SETTINGS_MANAGE（与促销面板同权限档）。
async fn require_manage(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = staff(req, state).await?;
    crate::authz::require_perm(
        state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    Ok(auth)
}

#[derive(Deserialize)]
pub struct ContestReq {
    pub title: String,
    pub descr: Option<String>,
    pub starts_at: Option<String>, // RFC3339
    pub ends_at: Option<String>,
    /// [{"rank":1,"sparks":5000},...]
    pub prizes: Option<serde_json::Value>,
}

/// GET /admin/contests —— 全部竞赛 + 报名数 + 当前分差榜头名。
#[get("/admin/contests")]
pub async fn admin_contests_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_manage(&req, &state).await?;
    let rows: Vec<(i32, String, Option<String>, bool, i64)> = sqlx::query_as(
        "SELECT c.id, c.title, c.descr, c.is_active, \
             (SELECT count(*) FROM contest_entries e \
              WHERE e.contest_id = c.id)::bigint \
         FROM contests c ORDER BY c.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!(rows
        .iter()
        .map(|(id, title, descr, active, joins)| serde_json::json!({
            "id": id, "title": title, "descr": descr,
            "is_active": active, "joins": joins,
        }))
        .collect::<Vec<_>>())))
}

/// POST /admin/contests —— 建赛。
#[post("/admin/contests")]
pub async fn admin_contests_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ContestReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_manage(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("竞赛标题不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO contests (title, descr, starts_at, ends_at, prizes, \
           is_active) \
         VALUES ($1, $2, \
           COALESCE($3::timestamptz, now()), \
           COALESCE($4::timestamptz, now() + interval '30 days'), \
           COALESCE($5, '[]'::jsonb), TRUE) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.descr)
    .bind(&body.starts_at)
    .bind(&body.ends_at)
    .bind(&body.prizes)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "contest.add", Some(i64::from(id)))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// PUT /admin/contests/{id} —— 编辑/开闭赛。
#[put("/admin/contests/{id}")]
pub async fn admin_contests_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<ContestReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_manage(&req, &state).await?;
    let id = path.into_inner();
    sqlx::query(
        "UPDATE contests SET title = $2, descr = $3, \
           starts_at = COALESCE($4::timestamptz, starts_at), \
           ends_at = COALESCE($5::timestamptz, ends_at), \
           prizes = COALESCE($6, prizes) \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.title.trim())
    .bind(&body.descr)
    .bind(&body.starts_at)
    .bind(&body.ends_at)
    .bind(&body.prizes)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "contest.update", Some(i64::from(id)))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// DELETE /admin/contests/{id} —— 关赛（置 is_active=false，保留历史）。
#[delete("/admin/contests/{id}")]
pub async fn admin_contests_close(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = require_manage(&req, &state).await?;
    let id = path.into_inner();
    sqlx::query("UPDATE contests SET is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "contest.close", Some(i64::from(id)))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

//! P3-16 Tracker URL 管理
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P3-16 Tracker URL 管理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TrackerUrlRow {
    id: i32,
    url: String,
    is_default: bool,
    enabled: bool,
    priority: i32,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/tracker-urls")]
async fn tracker_urls_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<TrackerUrlRow> = sqlx::query_as(
        "SELECT id, url, is_default, enabled, priority, \
         updated_at FROM tracker_urls ORDER BY priority, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct TrackerUrlReq {
    url: String,
    #[serde(default)]
    is_default: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    priority: Option<i32>,
}

#[post("/admin/tracker-urls")]
async fn tracker_url_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TrackerUrlReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TRACKER_MANAGE,
    )
    .await?;
    let url = body.url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(DomainError::Validation("URL 需以 http(s):// 开头".into()));
    }
    // 同 URL 不允许重复登记（announce 列表会重复下发同一地址）
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tracker_urls WHERE url = $1)",
    )
    .bind(url)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("该 Tracker URL 已存在".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO tracker_urls (url, is_default, enabled, priority) \
         VALUES ($1, COALESCE($2, FALSE), COALESCE($3, TRUE), COALESCE($4, 0)) \
         RETURNING id",
    )
    .bind(url)
    .bind(body.is_default)
    .bind(body.enabled)
    .bind(body.priority)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if body.is_default.unwrap_or(false) {
        sqlx::query(
            "UPDATE tracker_urls SET is_default = FALSE WHERE id <> $1",
        )
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "tracker_url.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/tracker-urls/{id}")]
async fn tracker_url_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<TrackerUrlReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TRACKER_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE tracker_urls SET url = $2, is_default = COALESCE($3, is_default), \
           enabled = COALESCE($4, enabled), priority = COALESCE($5, priority), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(body.url.trim())
    .bind(body.is_default)
    .bind(body.enabled)
    .bind(body.priority)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    if body.is_default.unwrap_or(false) {
        sqlx::query(
            "UPDATE tracker_urls SET is_default = FALSE WHERE id <> $1",
        )
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else if body.is_default == Some(false) {
        // 显式取消默认：不允许，否则站点可能没有默认 announce 地址
        return Err(DomainError::Validation(
            "不能直接取消默认地址，请把其他地址设为默认".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "tracker_url.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tracker-urls/{id}")]
async fn tracker_url_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TRACKER_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    // 删除后至少保留一条启用地址；默认地址不可直接删（先转移默认位再删）
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT is_default, enabled FROM tracker_urls WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((is_default, _enabled)) = row else {
        return Err(DomainError::NotFound(id as i64));
    };
    if is_default {
        return Err(DomainError::Validation(
            "默认地址不可删除，请先把其他地址设为默认".into(),
        ));
    }
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tracker_urls WHERE enabled AND id <> $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if left == 0 {
        return Err(DomainError::Validation(
            "至少保留一条启用的 Tracker 地址".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM tracker_urls WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "tracker_url.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

//! 置顶促销（第五轮 P2）：管理增删改 + 公开只读。
//! 从 admin_p2_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[get("/admin/sticky-promos")]
pub async fn sticky_promos_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<StickyPromoRow> = sqlx::query_as(
        "SELECT id, title, url, badge, starts_at, ends_at, enabled, sort \
         FROM sticky_promotions ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct StickyPromoReq {
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    badge: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[post("/admin/sticky-promos")]
pub async fn sticky_promos_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StickyPromoReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.title.trim().is_empty() || body.title.len() > 200 {
        return Err(DomainError::Validation("标题长度 1-200".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sticky_promotions (title, url, badge, sort, enabled, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(body.url.as_deref().map(str::trim))
    .bind(body.badge.as_deref())
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "sticky_promo.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/sticky-promos/{id}")]
pub async fn sticky_promos_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<StickyPromoReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE sticky_promotions SET \
            title = COALESCE($2, title), url = COALESCE($3, url), badge = COALESCE($4, badge), \
            sort = COALESCE($5, sort), enabled = COALESCE($6, enabled) \
         WHERE id = $1",
    )
    .bind(path_id)
    .bind(body.title.trim().to_string())
    .bind(body.url.clone())
    .bind(body.badge.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state
        .repo
        .audit(Some(auth.id), "sticky_promo.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/sticky-promos/{id}")]
pub async fn sticky_promos_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query("DELETE FROM sticky_promotions WHERE id = $1")
        .bind(path_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state
        .repo
        .audit(Some(auth.id), "sticky_promo.delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

/// 前台：生效中的置顶促销（时间窗内且启用）
#[get("/sticky-promos")]
pub async fn sticky_promos_public(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<StickyPromoRow> = sqlx::query_as(
        "SELECT id, title, url, badge, starts_at, ends_at, enabled, sort \
         FROM sticky_promotions \
         WHERE enabled AND starts_at <= now() AND ends_at > now() \
         ORDER BY sort, id LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- 自定义菜单（menu-items）----

#[derive(sqlx::FromRow, serde::Serialize)]
struct StickyPromoRow {
    id: i64,
    title: String,
    url: Option<String>,
    badge: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    enabled: bool,
    sort: i32,
}

//! P2-8 商店道具 CRUD
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItemRow {
    id: i64,
    name: String,
    kind: String,
    price: i64,
    config: serde_json::Value,
    active: bool,
}

#[get("/admin/shop-items")]
async fn admin_shop_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<ShopItemRow> = sqlx::query_as(
        "SELECT id, name, kind, price, config, \
         active FROM shop_items ORDER BY kind, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct ShopItemReq {
    name: String,
    kind: String,
    #[serde(default)]
    price: Option<i64>,
    #[serde(default)]
    config: Option<serde_json::Value>,
    #[serde(default)]
    active: Option<bool>,
}

#[post("/admin/shop-items")]
async fn admin_shop_item_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShopItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    if body.name.trim().is_empty() || body.kind.trim().is_empty() {
        return Err(DomainError::Validation("名称与类型必填".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO shop_items (name, kind, price, config, active) \
         VALUES ($1, $2, COALESCE($3, 0), COALESCE($4, '{}'::jsonb), COALESCE($5, TRUE)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "prop.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/shop-items/{id}")]
async fn admin_shop_item_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ShopItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE shop_items SET name = $2, kind = $3, price = COALESCE($4, price), \
           config = COALESCE($5, config), active = COALESCE($6, active) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "prop.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/shop-items/{id}")]
async fn admin_shop_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let id = path.into_inner();
    let in_use: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders WHERE item_id = $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if in_use > 0 {
        // 有持有/购买记录：只允许下架，不允许删（保留历史）
        let n =
            sqlx::query("UPDATE shop_items SET active = FALSE WHERE id = $1")
                .bind(id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
        state
            .repo
            .audit(Some(auth.id), "prop.disable", Some(id))
            .await;
        return Ok(ok(serde_json::json!({ "disabled": n > 0 })));
    }
    let n = sqlx::query("DELETE FROM shop_items WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "prop.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

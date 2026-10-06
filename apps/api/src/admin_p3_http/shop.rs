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
    /// 库存配额（0287）：NULL=不限；used 为已消耗数
    stock_quota: Option<i64>,
    stock_used: i64,
}

#[get("/admin/shop-items")]
async fn admin_shop_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<ShopItemRow> = sqlx::query_as(
        "SELECT id, name, kind, price, config, \
         active, stock_quota, stock_used \
         FROM shop_items ORDER BY kind, id",
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
    /// 库存配额（0287）：null=不限量；编辑时可同时改 stock_used（补纠错）
    #[serde(default)]
    stock_quota: Option<i64>,
    #[serde(default)]
    stock_used: Option<i64>,
}

/// 商品护栏（商城审计 P0-2/P1-3）：kind 必须在生效链白名单内（否则就是
/// 「花钱买空气」——测试架了 kind=free_money_forever 的 SKU 照样能上架）；
/// price 必须为正且封顶（负价商品 = 用户每买一单净入账，实测可刷）。
/// stock_used 不允许改成负数，quota 传 0 视为「清零限量=不限」由 SQL 层处理。
fn validate_shop_item(body: &ShopItemReq) -> DomainResult<()> {
    if body.name.trim().is_empty() || body.kind.trim().is_empty() {
        return Err(DomainError::Validation("名称与类型必填".into()));
    }
    if !crate::economy_http::has_effect(body.kind.trim()) {
        return Err(DomainError::Validation(format!(
            "未知商品类型 {}：必须在道具生效链白名单内，否则买到无效果",
            body.kind.trim()
        )));
    }
    let Some(price) = body.price else {
        return Ok(());
    };
    if !(1..=1_000_000_000).contains(&price) {
        return Err(DomainError::Validation(
            "价格须在 1 - 1,000,000,000 魔力之间".into(),
        ));
    }
    if let Some(used) = body.stock_used {
        if used < 0 {
            return Err(DomainError::Validation(
                "stock_used 不能为负数".into(),
            ));
        }
    }
    Ok(())
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
    validate_shop_item(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO shop_items \
         (name, kind, price, config, active, stock_quota, stock_used) \
         VALUES ($1, $2, COALESCE($3, 0), COALESCE($4, '{}'::jsonb), \
                 COALESCE($5, TRUE), $6, COALESCE($7, 0)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .bind(body.stock_quota)
    .bind(body.stock_used)
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
    validate_shop_item(&body)?;
    let id = path.into_inner();
    // P1-1（商城审计）：0287 的 stock_quota/stock_used 此前只在 INSERT 透传，
    // UPDATE 语句没 SET 这两列——后台库存编辑表单保存「成功」但库不落。
    // 现补齐：quota 传 null=改回不限量；used 仅显式传值时覆盖（补纠错口径）。
    let n = sqlx::query(
        "UPDATE shop_items SET name = $2, kind = $3, price = COALESCE($4, price), \
           config = COALESCE($5, config), active = COALESCE($6, active), \
           stock_quota = $7, \
           stock_used = COALESCE($8, stock_used) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .bind(body.stock_quota)
    .bind(body.stock_used)
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

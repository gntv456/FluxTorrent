//! 商城订单中心管理侧（商城审计 P2-1）：订单列表/筛选。
//! 与用户侧 my_orders.rs 同表，多三条运营维度：按用户/商品筛选、按
//! 效果发放态筛（effect_applied=false = 客服重点）、汇总口径（件数/流水）。
//! 退款不在本批：回收动线已有 /admin/users/{id}/revoke-item/{order_id}
//! （背包道具）与 /admin/user-vouchers/void（券），此处提供的是
//! 「找到那笔订单」的检索面，回收动作复用既有端点。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminOrderRow {
    id: i64,
    user_id: i64,
    #[sqlx(default)]
    username: Option<String>,
    item_id: i64,
    #[sqlx(default)]
    item_name: Option<String>,
    kind: String,
    price: i64,
    effect_applied: bool,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct AdminOrdersQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    /// 按用户筛选（精确 id 或用户名模糊）
    uid: Option<i64>,
    q: Option<String>,
    /// item_id 精确筛选
    item_id: Option<i64>,
    /// pending=true 只看效果未发放的（客服线索）
    pending: Option<bool>,
}

#[get("/admin/shop-orders")]
async fn admin_shop_orders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AdminOrdersQuery>,
) -> DomainResult<impl Responder> {
    let _auth = staff(&req, &state).await?;
    let page = q.page.unwrap_or(1).max(1);
    let per = q.per_page.unwrap_or(20).clamp(1, 50);
    let like = q.q.as_deref().map(|s| format!("%{}%", s.trim()));
    let rows: Vec<AdminOrderRow> = sqlx::query_as(
        "SELECT o.id, o.user_id, u.username, o.item_id, si.name AS item_name, \
         si.kind, o.price, o.effect_applied, o.idempotency_key, o.created_at \
         FROM shop_orders o \
         JOIN users u ON u.id = o.user_id \
         LEFT JOIN shop_items si ON si.id = o.item_id \
         WHERE ($1::bigint IS NULL OR o.user_id = $1) \
           AND ($2::text IS NULL OR u.username ILIKE $2) \
           AND ($3::bigint IS NULL OR o.item_id = $3) \
           AND ($4::bool IS NULL OR o.effect_applied = NOT $4) \
         ORDER BY o.id DESC LIMIT $5 OFFSET $6",
    )
    .bind(q.uid)
    .bind(&like)
    .bind(q.item_id)
    .bind(q.pending)
    .bind(per)
    .bind((page - 1) * per)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN users u ON u.id = o.user_id \
         WHERE ($1::bigint IS NULL OR o.user_id = $1) \
           AND ($2::text IS NULL OR u.username ILIKE $2) \
           AND ($3::bigint IS NULL OR o.item_id = $3) \
           AND ($4::bool IS NULL OR o.effect_applied = NOT $4)",
    )
    .bind(q.uid)
    .bind(&like)
    .bind(q.item_id)
    .bind(q.pending)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": page, "per_page": per,
    })))
}

//! 商城订单中心（商城审计 P2-1）：用户侧订单历史。
//! 之前 shop_orders 只写不读——用户买完只能去流水里看一排 kind='shop'，
//! 分不清买了什么。本端点按用户倒序分页返回订单（含商品名/kind/单价/
//! 效果发放态），管理员视角的列表在 admin 侧 shop_orders_admin.rs。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct MyOrderRow {
    id: i64,
    item_id: i64,
    /// 商品快照名（SKU 可能改名/下架，订单要显示当时的名字）
    #[sqlx(default)]
    item_name: Option<String>,
    kind: String,
    /// 订单行价格：普通类=总额，卡牌类=单价（多行拆单见 buy 的 ：N 键）
    price: i64,
    /// 效果是否已发放（False 长期存在=客服工单线索）
    effect_applied: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct MyOrdersQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[get("/me/orders")]
async fn my_orders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MyOrdersQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let page = q.page.unwrap_or(1).max(1);
    let per = q.per_page.unwrap_or(20).clamp(1, 50);
    let rows: Vec<MyOrderRow> = sqlx::query_as(
        "SELECT o.id, o.item_id, si.name AS item_name, si.kind, \
         o.price, o.effect_applied, o.created_at \
         FROM shop_orders o LEFT JOIN shop_items si ON si.id = o.item_id \
         WHERE o.user_id = $1 \
         ORDER BY o.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.id)
    .bind(per)
    .bind((page - 1) * per)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": page, "per_page": per,
    })))
}

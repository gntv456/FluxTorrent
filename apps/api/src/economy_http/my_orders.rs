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
    /// SKU 可能已被删（LEFT JOIN 给 NULL）：非 Option 读可空列必 500
    #[sqlx(default)]
    kind: Option<String>,
    /// 订单行价格：普通类=总额，卡牌类=单价（多行拆单见 buy 的 ：N 键）
    price: i64,
    /// 效果是否已发放（False 长期存在=客服工单线索）
    effect_applied: bool,
    /// 钱的阶段（0383）：reserving|frozen|done|failed|refunded。
    /// 与 effect_applied 是**两个正交判据**：前者说钱到哪一步，
    /// 后者说效果有没有落地，界面两列都要看。
    #[sqlx(default)]
    fulfill_state: Option<String>,
    #[sqlx(default)]
    fulfilled_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    fail_reason: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct MyOrdersQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    /// 履约态分组：pending|done|failed，其余（含缺省）= 全部
    #[serde(default)]
    fulfill: Option<String>,
}

/// 列表与计数共用：$2 为分组令牌，NULL 表示不筛；
/// 列表的 $3/$4 才是分页，所以计数只绑到 $2。
const ORDERS_WHERE: &str = " WHERE o.user_id = $1 \
    AND ($2::text IS NULL \
         OR ($2 = 'pending' \
             AND o.fulfill_state IN ('reserving', 'frozen')) \
         OR ($2 = 'done' AND o.fulfill_state = 'done') \
         OR ($2 = 'failed' \
             AND o.fulfill_state IN ('failed', 'refunded')))";

#[get("/me/orders")]
async fn my_orders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<MyOrdersQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 页码必须有上界：旧版只 max(1)，page=1e18 会让 OFFSET 乘出 i64 溢出，
    // 对外是一个 500 而不是一句「没有这一页」。
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per = q.per_page.unwrap_or(20).clamp(1, 50);
    let group = match q.fulfill.as_deref() {
        Some("pending") | Some("done") | Some("failed") => {
            q.fulfill.as_deref()
        }
        _ => None,
    };
    let rows: Vec<MyOrderRow> = sqlx::query_as(&format!(
        "SELECT o.id, o.item_id, si.name AS item_name, si.kind, \
         o.price, o.effect_applied, o.fulfill_state, o.fulfilled_at, \
         o.fail_reason, o.created_at \
         FROM shop_orders o LEFT JOIN shop_items si ON si.id = o.item_id \
         {ORDERS_WHERE} \
         ORDER BY o.id DESC LIMIT $3 OFFSET $4"
    ))
    .bind(auth.id)
    .bind(group)
    .bind(per)
    .bind((page - 1) * per)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM shop_orders o{ORDERS_WHERE}"
    ))
    .bind(auth.id)
    .bind(group)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": page, "per_page": per,
        "fulfill": group,
    })))
}

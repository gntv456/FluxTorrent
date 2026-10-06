//! P2-8 用户背包（shop_orders 口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{modify_log, staff};

#[derive(sqlx::FromRow, serde::Serialize)]
struct UserPropRow {
    order_id: i64,
    user_id: i64,
    username: String,
    item_id: i64,
    item_name: String,
    kind: String,
    price: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    /// 是否已生效（0291）：回收只对「0 价 + 未生效」的单成立，
    /// 列表不给这一列，站长只能点了才知道拒。
    effect_applied: bool,
}

#[derive(Deserialize)]
struct UserPropQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

/// 用户背包 = shop_orders（购买与发放统一落单；即时生效类不产生持有）
#[get("/admin/user-props")]
async fn admin_user_props(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserPropQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<UserPropRow> = sqlx::query_as(
        r#"SELECT o.id AS order_id, o.user_id, u.username, o.item_id,
                  i.name AS item_name, i.kind, o.price, o.created_at,
                  o.effect_applied
           FROM shop_orders o
           JOIN users u ON u.id = o.user_id
           JOIN shop_items i ON i.id = o.item_id
           WHERE ($1::bigint IS NULL OR o.user_id = $1)
           ORDER BY o.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o WHERE ($1::bigint IS NULL \
         OR o.user_id = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

/// 背包回收：删除未生效的 0 价发放单。
///
/// 0291：规则收进 `admin_http::reclaim_order`，与
/// `POST /admin/users/{id}/revoke-item/{order_id}` 共用一份。这条是后台背包面板
/// 实际调用的路径，此前它比那条弱得多——能删用户**付费购买**的单（白吞资产）、
/// 不退库存配额（永久少卖一格）、不过等级护栏（93 可删 99 的背包）。
/// 同一件事有两份规则时，界面在用的那份必然是被改漏的那份。
#[delete("/admin/user-props/{order_id}")]
async fn admin_user_prop_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let order_id = path.into_inner();
    let (uid, item_id, name) = crate::admin_http::reclaim_order(
        &state.repo.db,
        auth.class_id,
        order_id,
    )
    .await?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "prop.revoke",
            Some(uid),
            None,
            Some(serde_json::json!({
                "order_id": order_id, "item_id": item_id, "item_name": name,
            })),
        )
        .await;
    modify_log(
        &state.repo.db,
        uid,
        Some(auth.id),
        &format!("回收道具「{name}」"),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true, "order_id": order_id })))
}

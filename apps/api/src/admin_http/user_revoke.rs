//! 回收三件套（0287）+ 统一回收口径（0291）。
//!
//! 0291 起 `reclaim_order` 是**唯一**的背包回收实现：
//! `POST /admin/users/{id}/revoke-item/{order_id}` 与
//! 与 `DELETE /admin/user-props/{order_id}`（后台背包面板实际调的那条）共用同一套规则。
//! 此前两条路径规则不同——界面那条能删**用户花钱买的**单、不退库存配额、也不过等级护栏，
//! 而带校验的那条前端零调用。同一件事有两份规则，界面用的那份就必然是弱的那份。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 回收一单背包道具，返回 (目标用户, 道具 id, 道具名)。
///
/// 三道闸，缺一条就是「发错之后 Undo 反而造成第二次事故」：
/// - 付费单（price > 0）不在回收范围：那是退款，吞掉用户花魔力买的东西
///   既不退钱也不留凭证；
/// - 已生效单不可回收：即时类到账即生效，只能走调账冲销；
/// - 操作者等级须严格高于目标：否则 93 能删 99 的背包。
/// 删除与库存回滚同事务——配额没退回去就是永久少卖一格。
pub async fn reclaim_order(
    db: &PgPool,
    actor_class: i32,
    order_id: i64,
) -> DomainResult<(i64, i64, String)> {
    let row: Option<(i64, i64, String, i64, bool, i32)> = sqlx::query_as(
        "SELECT o.user_id, o.item_id, i.name, o.price, o.effect_applied, \
         u.class_id FROM shop_orders o \
         JOIN shop_items i ON i.id = o.item_id \
         JOIN users u ON u.id = o.user_id WHERE o.id = $1",
    )
    .bind(order_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, item_id, name, price, applied, target_class)) = row else {
        return Err(DomainError::NotFound(order_id));
    };
    if price > 0 {
        return Err(DomainError::Validation(
            "付费购买的道具单不属于回收范围（请走退款/调账，回收等于白吞）".into(),
        ));
    }
    if applied {
        return Err(DomainError::Validation(
            "该道具已生效，不可回收（即时类到账即生效，请走调账冲销）".into(),
        ));
    }
    if actor_class <= target_class {
        return Err(DomainError::Forbidden);
    }
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "DELETE FROM shop_orders WHERE id = $1 AND user_id = $2 AND price = 0 \
         AND NOT effect_applied",
    )
    .bind(order_id)
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "仅支持回收管理员发放（0 价未生效）的道具单".into(),
        ));
    }
    sqlx::query(
        "UPDATE shop_items SET stock_used = GREATEST(0, stock_used - 1) \
         WHERE id = $1 AND stock_quota IS NOT NULL",
    )
    .bind(item_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok((uid, item_id, name))
}

/// 回收背包道具（管理端旧路由，规则走 `reclaim_order`）。
#[post("/admin/users/{id}/revoke-item/{order_id}")]
pub(super) async fn user_revoke_item(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::PROP_MANAGE,
    )
    .await?;
    let (uid, order_id) = path.into_inner();
    let (_, item_id, _name) =
        reclaim_order(&state.repo.db, auth.class_id, order_id).await?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.revoke_item",
            Some(uid),
            None,
            Some(serde_json::json!({
                "order_id": order_id, "item_id": item_id,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "order_id": order_id })))
}


#[derive(Deserialize)]
pub(super) struct MedalRedateReq {
    /// 新有效期（天）：1-3650；null = 改为永久
    days: Option<i32>,
}

/// 勋章改期（0287 P1）：已持有者续期/缩短/转永久。
/// 单发 ON CONFLICT DO NOTHING 发不动已持有者——「活动勋章延期」
/// 这一最常见管理诉求此前无法表达，只能进数据库手改 expires_at。
#[post("/admin/users/{id}/medal/{medal_id}/redate")]
pub(super) async fn user_medal_redate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
    body: web::Json<MedalRedateReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, medal_id) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    if let Some(d) = body.days {
        if !(1..=3650).contains(&d) {
            return Err(DomainError::Validation(
                "days 需在 1-3650 之间（或不传=改为永久）".into(),
            ));
        }
    }
    let n = sqlx::query(
        "UPDATE user_medals SET expires_at = CASE WHEN $3::int IS NULL \
             THEN NULL ELSE now() + make_interval(days => $3::int) END \
         WHERE user_id = $1 AND medal_id = $2",
    )
    .bind(uid)
    .bind(medal_id)
    .bind(body.days)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(medal_id));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.medal_redate",
            Some(uid),
            None,
            Some(serde_json::json!({
                "medal_id": medal_id,
                "days": body.days,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "user_id": uid, "medal_id": medal_id })))
}

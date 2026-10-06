//! 回收三件套（0287 P1）：背包道具回收 / 券作废 / 勋章改期。
//! 发放面 0286 已补齐，这里补「发错之后的Undo」——三类资产只有勋章有
//! 回收端点，道具与券此前发出去就只能进数据库手改。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 回收背包道具：删掉未生效的 0 价发放单。
/// 只回收管理员发放（price=0 且 effect_applied=false）——已生效道具
/// （用户已用掉/已穿戴）不可逆，提示走人工调账；商店正常购买的单
/// 属退款范畴，不在本端点（价格≠0 不碰）。
#[post("/admin/users/{id}/revoke-item/{order_id}")]
pub(super) async fn user_revoke_item(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, order_id) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let order: Option<(i64, bool, String)> = sqlx::query_as(
        "SELECT item_id, effect_applied, coalesce(kind, '') \
         FROM shop_orders o LEFT JOIN shop_items i ON i.id = o.item_id \
         WHERE o.id = $1 AND o.user_id = $2",
    )
    .bind(order_id)
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((item_id, applied, _kind)) = order else {
        return Err(DomainError::NotFound(order_id));
    };
    if applied {
        return Err(DomainError::Validation(
            "该道具已生效，不可回收（即时类到账即生效，请走调账冲销）".into(),
        ));
    }
    let n = sqlx::query(
        "DELETE FROM shop_orders \
         WHERE id = $1 AND user_id = $2 AND price = 0 AND NOT effect_applied",
    )
    .bind(order_id)
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "仅支持回收管理员发放（0 价未生效）的道具单".into(),
        ));
    }
    // 库存回滚（0287 P3）：删单成功退回配额池（下限 0）
    sqlx::query(
        "UPDATE shop_items \
         SET stock_used = GREATEST(0, stock_used - 1) \
         WHERE id = $1 AND stock_quota IS NOT NULL",
    )
    .bind(item_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.revoke_item",
            Some(uid),
            None,
            Some(serde_json::json!({
                "order_id": order_id,
                "item_id": item_id,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "order_id": order_id })))
}

/// 券作废：未核销的管理员发放券可整批收回。
/// kind 维度作废（user_vouchers 无按张的管理语义——作废即把该用户
/// 该来源未用的券全删）；已核销（used_at 非空）的券动账走调账。
#[derive(Deserialize)]
pub(super) struct VoucherVoidReq {
    user_id: i64,
    /// 作废张数上限（1-50）：防误操作把某人全部券清掉时至少要先想清楚数量
    limit: i64,
}

#[post("/admin/user-vouchers/void")]
pub(super) async fn admin_voucher_void(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherVoidReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    if !(1..=50).contains(&body.limit) {
        return Err(DomainError::Validation("limit 需在 1-50".into()));
    }
    let n = sqlx::query(
        "DELETE FROM user_vouchers WHERE id IN ( \
            SELECT id FROM user_vouchers WHERE user_id = $1 \
            AND used_at IS NULL AND used_torrent_id IS NULL \
            ORDER BY id LIMIT $2)",
    )
    .bind(body.user_id)
    .bind(body.limit)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    // 库存回滚（0287 P3）：作废的券退回道具池（按免费券道具行；其它来源券无池）
    sqlx::query(
        "UPDATE shop_items \
         SET stock_used = GREATEST(0, stock_used - $2) \
         WHERE kind IN ('voucher_free','voucher_neutral') \
           AND active AND stock_quota IS NOT NULL",
    )
    .bind(n as i64)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "voucher.void",
            Some(body.user_id),
            None,
            Some(serde_json::json!({ "voided": n })),
        )
        .await;
    Ok(ok(serde_json::json!({ "voided": n })))
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
    Ok(ok(
        serde_json::json!({ "user_id": uid, "medal_id": medal_id }),
    ))
}

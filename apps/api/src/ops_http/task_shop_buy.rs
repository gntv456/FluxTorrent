//! 任务商店兑换端点（0115，五问评审 P1 收口）。
//! 门槛/库存/扣款/发券按既有商城语义编排：事务内占位+扣款+建单，
//! 事务外发券+CAS 标记；重放时只补发未生效订单，不重复扣款占库存。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuyReq {
    item_id: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/tasks/shop/buy")]
async fn task_shop_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let raw_key = crate::http::idempotency::body_or_header(&body.idempotency_key)
        .filter(|k| crate::http::idempotency::valid_shape(k))
        .ok_or_else(|| DomainError::Validation("缺少合法的 idempotency_key".into()))?;
    let idem = format!("task_shop:{}:{}", auth.id, raw_key);

    let item = crate::repo::task_shop::item_for_buy(&state, body.item_id)
        .await?
        .ok_or_else(|| DomainError::NotFound(body.item_id))?;
    // 无发放臂的商品（如「射魔一次」）拒单：不许扣款发空气
    let kind = item
        .kind
        .clone()
        .filter(|k| crate::economy_http::has_effect(k) || matches!(k.as_str(), "temp_invite" | "rename_card"))
        .ok_or_else(|| DomainError::Validation("该权益兑换暂未开放".into()))?;

    // 重放：已扣款订单按 effect_applied 决定补发还是原样返回
    if let Some((order_id, applied, _, _)) =
        crate::repo::task_shop::order_by_idem(&state, auth.id, &idem).await?
    {
        if applied {
            return Ok(ok(serde_json::json!({
                "replayed": true,
                "order_id": order_id,
                "item_id": body.item_id,
            })));
        }
        // 补发：扣款与库存都不再动，只把没发出去的权益补上
        crate::economy_http::apply_item_effect(
            &state.repo.db,
            auth.id,
            &kind,
            &item.config,
            "task_shop",
        )
        .await?;
        crate::repo::task_shop::mark_effect_applied(&state, order_id).await?;
        return Ok(ok(serde_json::json!({
            "replayed": true,
            "order_id": order_id,
            "item_id": body.item_id,
            "redelivered": true,
        })));
    }

    // 门槛：该难度档已完成数
    let done = crate::repo::task_shop::tier_done_count(
        &state,
        auth.id,
        &item.require_tier,
    )
    .await?;
    if done < i64::from(item.require_count) {
        return Err(DomainError::Validation(format!(
            "需在 {} 档完成 {} 个任务（当前 {}）",
            item.require_tier, item.require_count, done
        )));
    }
    // 库存
    if item.stock > 0 && item.stock_used >= item.stock {
        return Err(DomainError::Validation("该权益已兑完".into()));
    }
    // 魔力取整：cost 存 NUMERIC（1.0），用户可见价格即四舍五入值
    let cost = item.cost.round() as i64;
    if cost < 0 {
        return Err(DomainError::Validation("商品价格配置异常".into()));
    }

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !crate::repo::task_shop::take_stock(&mut tx, body.item_id).await? {
        return Err(DomainError::Validation("该权益已兑完".into()));
    }
    let outcome = crate::economy_http::spend_spark_tx(
        &mut tx,
        auth.id,
        cost,
        "task_shop",
        &idem,
        "task_shop_item",
        body.item_id,
    )
    .await?;
    if matches!(outcome, crate::economy_http::SpendOutcome::Replayed) {
        // 极小并发窗口：同键已由另一请求扣款；归还本次占位
        crate::repo::task_shop::release_stock(&state.repo.db, body.item_id).await?;
        return Err(DomainError::Validation("该笔兑换已受理，请刷新后查看".into()));
    }
    let order_id = crate::repo::task_shop::insert_order(
        &mut tx,
        auth.id,
        body.item_id,
        cost,
        &idem,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 发券在事务外（共享发放臂绑定 PgPool）：失败留 effect_applied=false，
    // 同键重试命中上面的补发分支，不会二次扣款。
    let applied = crate::economy_http::apply_item_effect(
        &state.repo.db,
        auth.id,
        &kind,
        &item.config,
        "task_shop",
    )
    .await
    .is_ok();
    if applied {
        crate::repo::task_shop::mark_effect_applied(&state, order_id).await?;
    }
    state.repo.audit(
        Some(auth.id),
        "task_shop_buy",
        Some(body.item_id),
    ).await;

    Ok(ok(serde_json::json!({
        "order_id": order_id,
        "item_id": body.item_id,
        "cost": cost,
        "name": item.name,
        "effect_applied": applied,
    })))
}

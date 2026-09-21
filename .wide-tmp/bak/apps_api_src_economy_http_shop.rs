//! 商店（M11）：道具列表/购买/生效。
//! 从 economy_http.rs 按域拆出。

use super::shop_effects::apply_item_effect;
use super::spend::spend_spark;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 商店（M11） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItem {
    id: i64,
    name: String,
    kind: String,
    price: i64,
}

#[get("/shop/items")]
async fn shop_items(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let items = sqlx::query_as::<_, ShopItem>(
        "SELECT id, name, kind, price FROM shop_items WHERE active = true ORDER BY price",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(items))
}

#[derive(Deserialize)]
struct BuyReq {
    item_id: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/shop/buy")]
async fn shop_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let item: Option<(String, String, i64, serde_json::Value)> = sqlx::query_as(
        "SELECT name, kind, price, config FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, price, config)) = item else {
        return Err(DomainError::NotFound(body.item_id));
    };

    // 审计修复（P1 花钱买空气）：唯一性道具（装扮/头衔类）重复购买此前照扣全价、
    // 效果 ON CONFLICT DO NOTHING —— 已拥有者再买 = 花钱买空气。可叠加类
    //（邀请/券/上传量/VIP 时长）不在此列。拥有判定与 apply_item_effect 同表。
    if matches!(
        kind.as_str(),
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name"
    ) {
        let item_id = config.get("item_id").and_then(|v| v.as_i64());
        if let Some(iid) = item_id {
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_dressups WHERE user_id = $1 AND item_id = $2)",
            )
            .bind(auth.id)
            .bind(iid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
            if owned {
                return Err(DomainError::Validation(
                    "你已拥有该装扮，无需重复购买（装扮类道具不叠加）".into(),
                ));
            }
        }
    }

    // 幂等键必填（P1）：网络层重试必须携带同一键，否则双扣款。
    // 服务端键必须带 uid 前缀：裸客户端键跨用户碰撞时，B 的消费会被误判为 A 的重放。
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.is_empty())
        .map(|k| format!("shop:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "shop_item",
        body.item_id,
    )
    .await?;

    // 订单落库（幂等键唯一）
    sqlx::query(
        "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .bind(price)
    .bind(&idem)
    .bind(config.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 效果执行（CAS 置位，0085）：首次成功扣款 OR 重试补发（此前 Replayed 直接跳过
    // 效果分支——扣款成功但效果失败后重试 = 花钱买空气）。置位失败 = 效果已发过，跳过。
    let _ = outcome;
    let should_apply: Option<i64> = sqlx::query_scalar(
        "UPDATE shop_orders SET effect_applied = TRUE          WHERE user_id = $1 AND idempotency_key = $2 AND NOT effect_applied RETURNING id",
    )
    .bind(auth.id)
    .bind(&idem)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if should_apply.is_some() {
        let mut cfg = config.clone();
        cfg["item_id"] = serde_json::json!(body.item_id);
        apply_item_effect(&state.repo.db, auth.id, &kind, &cfg).await?;
    }

    state
        .repo
        .audit(Some(auth.id), "shop_buy", Some(body.item_id))
        .await;
    Ok(ok(
        serde_json::json!({ "item": name, "price": price, "idempotency_key": idem }),
    ))
}

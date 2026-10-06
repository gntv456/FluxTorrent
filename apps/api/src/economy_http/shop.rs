//! 商店（M11）：道具列表/购买/生效。
//! 从 economy_http.rs 按域拆出。
//! 0207：购买支持数量（stackable 类）；装扮类 SKU 拆分后 config 驱动候选选择。

use super::shop_effects::apply_item_effect;
use super::shop_list::buffer_cap_check;
use super::shop_list::shop_items;
use super::spend::spend_spark;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
struct BuyReq {
    item_id: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
    /// 购买数量（0207）：仅 stackable 类生效；装扮/头衔类强制 1
    #[serde(default)]
    qty: Option<i64>,
}

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 商店（M11） ============

#[post("/shop/buy")]
async fn shop_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let item: Option<(String, String, i64, serde_json::Value)> =
        sqlx::query_as(
            "SELECT name, kind, price, \
         config FROM shop_items WHERE id = $1 AND active = true",
        )
        .bind(body.item_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, unit_price, config)) = item else {
        return Err(DomainError::NotFound(body.item_id));
    };

    // 数量口径（0207）：stackable 类可 1..=100；其余一律 1。
    // 唯一性道具（装扮/头衔类）叠加购买 = 花钱买空气，由 stackable 缺省挡住。
    let stackable = config
        .get("stackable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let qty = body.qty.unwrap_or(1);
    if qty < 1 || qty > 100 {
        return Err(DomainError::Validation("购买数量须在 1-100 之间".into()));
    }
    if qty > 1 && !stackable {
        return Err(DomainError::Validation(
            "该商品不可叠加购买，数量固定为 1".into(),
        ));
    }
    let total = unit_price
        .checked_mul(qty)
        .ok_or(DomainError::Validation("数量超出可计算范围".into()))?;

    // 审计修复（P1 花钱买空气）：唯一性道具（装扮/头衔类）重复购买此前照扣全价、
    // 效果 ON CONFLICT DO NOTHING —— 已拥有者再买 = 花钱买空气。可叠加类
    //（邀请/券/上传量/VIP 时长）不在此列。拥有判定与 apply_item_effect 同表
    //（0207 起直接按 shop_items.id 判，旧口径读 config.item_id 对种子 SKU 永远
    // 为空、护栏从未生效）。
    if matches!(
        kind.as_str(),
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name"
    ) {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_dressups \
             WHERE user_id = $1 AND item_id = $2)",
        )
        .bind(auth.id)
        .bind(body.item_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if owned {
            return Err(DomainError::Validation(
                "你已拥有该装扮，无需重复购买（装扮类道具不叠加）".into(),
            ));
        }
    }

    if matches!(kind.as_str(), "upload_credit" | "upload" | "upload_gb") {
        buffer_cap_check(&state.repo.db, auth.id).await?;
    }

    // 库存配额（0287 P3）：quota 非空时 buy 前原子占位——
    // UPDATE ... WHERE stock_used < stock_quota 抢不到即售罄，防超发
    let stocked: Option<bool> = sqlx::query_scalar(
                "UPDATE shop_items SET stock_used = stock_used + $2 \
         WHERE id = $1 AND stock_quota IS NOT NULL \
         RETURNING (stock_used <= stock_quota)",
    )
    .bind(body.item_id)
    .bind(qty as i64)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(within) = stocked {
        if !within {
            // 抢到了但越界：回滚占位再报售罄
            sqlx::query(
                            "UPDATE shop_items \
             SET stock_used = stock_used - $2 \
             WHERE id = $1",
            )
            .bind(body.item_id)
            .bind(qty as i64)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            return Err(DomainError::Validation("该道具已售罄".into()));
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
        total,
        "shop",
        &idem,
        "shop_item",
        body.item_id,
    )
    .await?;

    // 重放＝这个请求已经处理过：既不能再插订单行，也不能再发效果。
    // 旧版这里写 `let _ = outcome;` 把 Replayed 吞掉继续往下走，而卡牌类
    // （补签/改名/临时邀请）会用 `idem:2..idem:N` 的新行绕过扣款——
    // 同一 idempotency_key 先 qty=1 买一次、再 qty=100 买一次，
    // 第二次分文不花却领到 99 张券（券可兑 30 天邀请 → 批量开小号）。
    if matches!(outcome, crate::economy_http::SpendOutcome::Replayed) {
        let orders: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop_orders WHERE user_id = $1 \
             AND (idempotency_key = $2 OR idempotency_key LIKE $3)",
        )
        .bind(auth.id)
        .bind(&idem)
        .bind(format!("{}:%", idem))
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        tracing::info!(
            user = auth.id,
            item = body.item_id,
            "商店购买重放，按原单返回"
        );
        return Ok(ok(serde_json::json!({
            "replayed": true,
            "item_id": body.item_id,
            "orders": orders,
        })));
    }

    // 订单落库（幂等键唯一）。卡牌类（补签/改名/临时邀请）库存口径 = 订单行数
    //（checkin/gaps 按「无 resub_uses 的订单」计数），因此 qty 张就插 qty 行
    //（键加序号后缀保持幂等）；其余类一行、price 记总额（0207）。
    let card_kind =
        matches!(kind.as_str(), "makeup_card" | "rename_card" | "temp_invite");
    if card_kind && qty > 1 {
        for k in 1..=qty {
            let key = if k == 1 {
                idem.clone()
            } else {
                format!("{}:{}", idem, k)
            };
            sqlx::query(
                "INSERT INTO shop_orders (user_id, item_id, price, \
                 idempotency_key, config_snapshot) VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (idempotency_key) DO NOTHING",
            )
            .bind(auth.id)
            .bind(body.item_id)
            .bind(unit_price)
            .bind(&key)
            .bind(config.clone())
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    } else {
        sqlx::query(
            "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (idempotency_key) DO NOTHING",
        )
        .bind(auth.id)
        .bind(body.item_id)
        .bind(total)
        .bind(&idem)
        .bind(config.clone())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 效果执行（CAS 置位，0085）：首次成功扣款 OR 重试补发（此前 Replayed 直接跳过
    // 效果分支——扣款成功但效果失败后重试 = 花钱买空气）。置位失败 = 效果已发过，跳过。
    // 卡牌多行订单：逐行 CAS，每次置位成功发一轮效果。
    let mut applied = 0usize;
    loop {
        let hit: Option<i64> = sqlx::query_scalar(
            "UPDATE shop_orders SET effect_applied = TRUE WHERE id IN ( \
             SELECT id FROM shop_orders WHERE user_id = $1 \
             AND (idempotency_key = $2 OR idempotency_key LIKE $3) \
             AND NOT effect_applied ORDER BY id LIMIT 1) RETURNING id",
        )
        .bind(auth.id)
        .bind(&idem)
        .bind(format!("{}:%", idem))
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if hit.is_none() {
            break;
        }
        applied += 1;
        // 数量类效果按件发（上传量×N、券×N、邀请×N、卡牌入库×N）
        let mut cfg = config.clone();
        cfg["item_id"] = serde_json::json!(body.item_id);
        apply_item_effect(&state.repo.db, auth.id, &kind, &cfg, "buy").await?;
        if !card_kind {
            break; // 非卡牌：单行订单，一轮即全部效果（qty 循环见下）
        }
    }
    if !card_kind && applied > 0 {
        let rounds = if stackable { qty } else { 1 };
        let mut cfg = config.clone();
        cfg["item_id"] = serde_json::json!(body.item_id);
        for _ in 1..rounds {
            apply_item_effect(&state.repo.db, auth.id, &kind, &cfg, "buy")
                .await?;
        }
    }

    state
        .repo
        .audit(Some(auth.id), "shop_buy", Some(body.item_id))
        .await;
    Ok(ok(serde_json::json!({
        "item": name, "price": total, "qty": qty,
        "idempotency_key": idem,
    })))
}

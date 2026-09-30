//! 农场动作（M24）：种植/浇水/收获。
//! 从 games_http/farm.rs 按域拆出；作物行/限次助手仍经 farm.rs 与 helpers.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{
    earn_spark, earn_spark_tx, spend_spark, SpendOutcome,
};
use crate::errors::{DomainError, DomainResult};
use crate::games;

use super::farm_egg::roll_egg;
use crate::http::require_auth;
use crate::state::AppState;

use super::farm::{get_crop, get_plantable_crop, PlantReq};
use super::helpers::{
    check_rate_scoped, eco_i64, farm_market_hours, farm_wither_days, RateScope,
};

#[post("/farm/plant")]
pub(super) async fn farm_plant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PlantReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 审计修复（P1 印钞）：农场种植此前不进 games 限流（对照 scratch/dice/jgg），
    // 确定性市场价可被脚本以 6 槽 × 高频轮种套取波动收益。现独立计数（rl:farm），
    // 与即时赌局额度分开 —— 否则种满 6 块地就吃掉 6 次下注额度。
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Farm).await?;
    let crop = get_plantable_crop(&state.repo.db, body.crop_id).await?;
    // 地块关闸：槽位必须是我**持有**的（免费 6 块 + 按阶梯买来的），
    // 成熟分钟数由那一块的等级定（升级只买周转，不改产量 —— games::farm_land）。
    let minutes = super::farm_land::plant_guard(
        &state, auth.id, body.slot, crop.grow_hours,
    )
    .await?;

    let now = chrono::Utc::now().timestamp();
    let window =
        games::market_window_start_with(now, farm_market_hours(&state).await);
    let price = games::market_price(crop.seed_price as i64, window);

    // 买种经统一交易管线扣款（幂等键含用户+槽位+当前分钟）。
    // 审计修复（P0 铸币）：旧逻辑对 spend_spark 返回的 Replayed 不检查——同槽同分钟内
    // 第二个请求（换高价作物）不扣款即走到占位失败分支，再按"本次请求的高价"全额退款。
    // 现在：Replayed 直接拒绝（本请求未付费），退款金额以幂等键对应的实际扣款额为准。
    let idem = format!("farm-plant:{}:{}:{}", auth.id, body.slot, now / 60);
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "game",
        &idem,
        "farm_plant",
        crop.id as i64,
    )
    .await?;
    if !matches!(outcome, SpendOutcome::Spent) {
        return Err(DomainError::Validation(
            "操作过于频繁，请一分钟后再试".into(),
        ));
    }

    let planted = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO farm_plots (user_id, slot, crop_id, ready_at)
         VALUES ($1, $2, $3, now() + make_interval(mins => $4::int))
         ON CONFLICT (user_id, slot) DO NOTHING RETURNING id"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(crop.id)
    .bind(minutes)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if planted.is_none() {
        // 占位失败（槽位已被占）：退款补偿。退款额必须取该幂等键的实际扣款额，
        // 而非本次请求价——两者在本请求 Replayed 已被拒绝的前提下仍可能有差异
        //（同分钟内首请求是低价作物），按实际扣款退才能保证净 0。
        let actual: Option<i64> = sqlx::query_scalar(
            "SELECT amount FROM spark_ledger WHERE idempotency_key \
             = $1 AND user_id = $2 LIMIT 1",
        )
        .bind(&idem)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let refund_amt = actual.unwrap_or(price);
        let refund = format!("farm-refund:{}", idem);
        earn_spark(&state.repo.db, auth.id, refund_amt, "game", &refund)
            .await?;
        return Err(DomainError::Validation("该地块已有作物".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "farm.plant", Some(crop.id as i64))
        .await;
    let ready = chrono::Utc::now() + chrono::Duration::minutes(minutes);
    Ok(ok(serde_json::json!({
        "slot": body.slot, "crop": crop.name, "cost": price,
        "minutes": minutes,
        "ready_at": ready.to_rfc3339(),
    })))
}

#[derive(Deserialize)]
struct SlotReq {
    slot: i32,
}

#[post("/farm/water")]
pub(super) async fn farm_water(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 枯萎地块不能浇水（有效期已过，救不回来）
    let wither_days = farm_wither_days(&state).await;
    let updated = sqlx::query(
        r#"UPDATE farm_plots SET watered = TRUE, ready_at = ready_at - interval '10 minutes'
         WHERE user_id = $1 AND slot = $2 AND watered = FALSE
           AND ($3 = 0 OR ready_at + make_interval(days => $3::int) >= now())"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(wither_days)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "该地块无需浇水（未种植 / 已浇过 / 已枯萎）".into(),
        ));
    }
    // 浇水消耗（0109 键 farm_water_spark，缺省 1；为 0 表示免费）。
    // 先占位后扣费，扣费失败回滚占位（与 fun_vote 同口径，避免白扣或白浇）。
    let cost = eco_i64(&state, "farm_water_spark", 1).await;
    if cost > 0 {
        let idem = format!("farm-water:{}:{}", auth.id, body.slot);
        if let Err(e) = spend_spark(
            &state.repo.db,
            auth.id,
            cost,
            "game",
            &idem,
            "farm_water",
            body.slot as i64,
        )
        .await
        {
            let _ = sqlx::query(
                r#"UPDATE farm_plots SET watered = FALSE, ready_at = ready_at + interval '10 minutes'
                   WHERE user_id = $1 AND slot = $2"#,
            )
            .bind(auth.id)
            .bind(body.slot)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    }
    Ok(ok(
        serde_json::json!({ "watered": true, "accelerated_minutes": 10, "cost": cost }),
    ))
}

#[post("/farm/harvest")]
pub(super) async fn farm_harvest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let wither_days = farm_wither_days(&state).await;
    // 单事务（P1 收口）：地块行锁、入账、清地块、留痕同生共死。旧版 FOR UPDATE 用在
    // autocommit 连接上锁立即失效（重复收获实际只靠 earn 幂等键兜底），且 earn 与
    // DELETE/留痕分属多个事务，中途失败会留下「钱发了地还在/地没了账没记」的中间态。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 行级锁防重复收获（先 FOR UPDATE 拿到成熟地块）；`FOR UPDATE OF p` 只锁地块表，
    // 不锁 crops（否则所有收同一作物的用户会互相串行）
    let plot: Option<(i64, i32, String, bool)> = sqlx::query_as(
        r#"SELECT p.id, p.crop_id, c.name AS crop_name,
             ($3 > 0 AND p.ready_at + make_interval(days => $3::int) < now()) AS withered
         FROM farm_plots p JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 AND p.slot = $2 AND p.ready_at <= now() FOR UPDATE OF p"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(wither_days)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plot_id, crop_id, crop_name, withered)) = plot else {
        return Err(DomainError::Validation("该地块尚未成熟".into()));
    };

    // 枯萎（超过有效期未收）：收获作废（0 魔力），但**清空地块**让玩家能重种。
    // 记账留痕（amount=0 的收获流水），便于运营统计浪费。
    if withered {
        sqlx::query("DELETE FROM farm_plots WHERE id = $1")
            .bind(plot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO farm_harvests (user_id, crop_id, amount, \
             market_price, doubled) VALUES ($1, $2, 0, 0, FALSE)",
        )
        .bind(auth.id)
        .bind(crop_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        state
            .repo
            .audit(Some(auth.id), "farm.wither", Some(crop_id as i64))
            .await;
        return Ok(ok(serde_json::json!({
            "crop": crop_name, "amount": 0, "market_price": 0, "doubled": false, "withered": true,
        })));
    }

    let now = chrono::Utc::now().timestamp();
    let window =
        games::market_window_start_with(now, farm_market_hours(&state).await);
    let crop = get_crop(&state.repo.db, crop_id)
        .await?
        .ok_or(DomainError::Validation("作物不存在".into()))?;
    // 收获量 = base_yield × 收获侧市场因子（±50% 窗口波动；与买种侧因子错开——
    // 见 games::harvest_market_price 注释，消除确定性低买高卖套利）。
    // 回收口径：作物表按「产量 = 种子价 × 0.75」标定，含 20% 双倍后期望回报 0.90 < 1
    // （0127 迁移统一下发，五档一致；市场 ±50% 只影响单局运气，不改期望）。
    let market = games::harvest_market_price(crop.base_yield as i64, window);

    let doubled = games::roll_double();
    let amount = if doubled { market * 2 } else { market };

    // 收获彩蛋：额外一档来自 arcade_pools(game='farm')，魔力按**这一株的种子价**
    // 倍数派，或直接发一件目录里的物品。基础收获仍是确定性的 —— 彩蛋是站长
    // 想加才加的东西，写侧与运行时都按「0.90 + 彩蛋 < 1」把关（validate_farm）。
    let egg = roll_egg(&state.repo.db, crop.seed_price as i64).await?;
    let extra = egg.as_ref().map(|e| e.extra).unwrap_or(0);
    let total = amount.saturating_add(extra);

    // 收益经统一交易管线入账（幂等键绑定地块；与地块锁同事务，双重防重复收获。
    // 地块已在本事务锁定且即将删除，重放不可达；显式丢弃以满足 must_use 契约）
    let idem = format!("farm-harvest:{}", plot_id);
    if total > 0 {
        let _ = earn_spark_tx(&mut tx, auth.id, total, "game", &idem).await?;
    }

    sqlx::query("DELETE FROM farm_plots WHERE id = $1")
        .bind(plot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO farm_harvests (user_id, crop_id, amount, \
         market_price, doubled) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(auth.id)
    .bind(crop_id)
    .bind(total)
    .bind(market)
    .bind(doubled)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 物品彩蛋在提交之后发：地块与账已经落定，发放本身自带幂等与库存判定
    let award = match egg {
        Some(e) => e.settle(&state.repo.db, auth.id, plot_id).await?,
        None => serde_json::Value::Null,
    };

    Ok(ok(serde_json::json!({
        "crop": crop.name, "amount": total, "base": amount,
        "extra": extra, "market_price": market, "doubled": doubled,
        "withered": false, "prize": award,
    })))
}

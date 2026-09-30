//! 农场土地阶梯（写侧）：买地 + 升级。两个动作都是确定性魔力沉没口，
//! 报价与档位一律取自 `farm_land::LandState`（读侧单源），这里只动账与落行。
//!
//! 并发口径：`spend_spark_tx` 先取用户行锁，之后所有复判（档数、等级）都在
//! 同一把锁之后；扣款与落行走同一事务，任何一步失败整体回滚 —— 不会出现
//! 「钱扣了地没到手」。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games::{
    speed_permille, validate_purchase, validate_upgrade, MAX_LEVEL,
};
use crate::http::require_auth;
use crate::state::AppState;

use super::farm_land::{land_err, load_land};
use super::helpers::{check_rate_scoped, RateScope};

#[derive(Deserialize)]
struct SlotReq {
    slot: i32,
}

#[post("/farm/land/buy")]
pub(super) async fn farm_land_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Farm).await?;
    let land = load_land(&state, auth.id).await?;
    let owned = land.owned();
    let cap = land.cap();
    let slot = validate_purchase(
        body.slot,
        land.free(),
        land.purchased(),
        cap,
    )
    .map_err(land_err)?;
    let price = land.land_quote();

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = format!("farm-land:{}:{}", auth.id, slot);
    let outcome = spend_spark_tx(
        &mut tx,
        auth.id,
        price,
        "game",
        &idem,
        "farm_land",
        slot as i64,
    )
    .await?;
    if !matches!(outcome, SpendOutcome::Spent) {
        return Err(land_err("操作过于频繁，请稍后再试"));
    }
    // 出价用的档数是事务外读的：拿到用户行锁之后必须复一次，否则两个并发
    // 请求会按同一个价各买一块（阶梯被打折），或买满后还成交。
    let now_purchased = purchased_count(&mut tx, auth.id, land.free()).await?;
    if now_purchased != land.purchased() {
        return Err(land_err(
            "地块档数刚发生变化（可能你自己正在买），请刷新后重试",
        ));
    }
    let ins = sqlx::query(
        "INSERT INTO farm_land (user_id, slot, level) VALUES ($1, $2, 1)",
    )
    .bind(auth.id)
    .bind(slot)
    .execute(&mut *tx)
    .await;
    if let Err(e) = ins {
        return Err(match e {
            sqlx::Error::Database(d)
                if d.code().as_deref() == Some("23505") =>
            {
                land_err(format!("第 {slot} 号地块你已经有了，不用重复买"))
            }
            other => DomainError::Internal(other.into()),
        });
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "farm.land_buy", Some(slot as i64))
        .await;
    Ok(ok(serde_json::json!({
        "slot": slot, "level": 1, "cost": price,
        "owned": owned + 1, "cap": cap,
    })))
}

#[post("/farm/land/upgrade")]
pub(super) async fn farm_land_upgrade(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Farm).await?;
    let land = load_land(&state, auth.id).await?;
    if body.slot < 1 || body.slot > land.owned() {
        return Err(land_err(format!(
            "第 {} 号地块还不是你的，升不了级",
            body.slot
        )));
    }
    let from = land.level(body.slot);
    validate_upgrade(from).map_err(land_err)?;
    let price = land.up_quote(from);

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等键**绑在「从第几级升上去」**：同一级的重放必然被拒（不重复扣钱），
    // 而真正的下一次升级键自然不同 —— 这比按分钟取窗口更准。
    let idem = format!("farm-up:{}:{}:{}", auth.id, body.slot, from);
    let outcome = spend_spark_tx(
        &mut tx,
        auth.id,
        price,
        "game",
        &idem,
        "farm_up",
        body.slot as i64,
    )
    .await?;
    if !matches!(outcome, SpendOutcome::Spent) {
        return Err(land_err(
            "这一级的升级费刚刚已经付过一次，请刷新后重试",
        ));
    }
    // 用户行锁已由 spend_spark_tx 取到，这里的等级复判不会再被并发插队
    let held: Option<i32> = sqlx::query_scalar(
        "SELECT level FROM farm_land WHERE user_id = $1 AND slot = $2 \
         FOR UPDATE",
    )
    .bind(auth.id)
    .bind(body.slot)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let actual = held.unwrap_or(1);
    if actual != from {
        return Err(land_err(
            "地块等级已变化（并发升级），本次扣款已回滚，请刷新后重试",
        ));
    }
    let done = sqlx::query(
        "UPDATE farm_land SET level = $3, upgraded_at = now() \
         WHERE user_id = $1 AND slot = $2",
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(from + 1)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if done.rows_affected() == 0 {
        // 免费地块的持有行只在第一次升级时出现（level>1 才落行，见 CHECK）
        sqlx::query(
            "INSERT INTO farm_land (user_id, slot, level, upgraded_at) \
             VALUES ($1, $2, $3, now())",
        )
        .bind(auth.id)
        .bind(body.slot)
        .bind(from + 1)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "farm.land_up", Some(body.slot as i64))
        .await;
    Ok(ok(serde_json::json!({
        "slot": body.slot, "level": from + 1, "cost": price,
        "speed_permille": speed_permille(from + 1),
        "max_level": MAX_LEVEL,
    })))
}

/// 已买块数（在事务里、用户行锁之后复算）
async fn purchased_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    free: i32,
) -> DomainResult<i32> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM farm_land WHERE user_id = $1 AND slot > $2",
    )
    .bind(user_id)
    .bind(free)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n as i32)
}

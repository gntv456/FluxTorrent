//! 宠物养成（娱乐屋第 9 款）。
//!
//! 经济定位：**回收口**。宠物「卖萌」产出的魔力只来自你喂进去的「能量」，且返回
//! 比例恒 < 1000‰（等级越高封顶 950‰）—— 最多把你喂的 95% 还给你，永不增发。
//! 它的意义是**养成**（等级/外观），不是套利。
//!
//! 时间推进不依赖 worker：每次读取/喂食/领取，按 `last_tick` → 此刻的间隔结算一次
//! （消化能量进 pending、衰减饥饿）。领取的入账与 pending 清零在**同一事务**内，
//! 行锁串行化并发领取，幂等键挡重放。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::food_coupon::pay_feed_tx;
use super::helpers::{check_rate_scoped, eco_i64, idem_key, RateScope};
use super::pool::dberr;

const MAX_LEVEL: i32 = 10;
const EXP_PER_LEVEL: i64 = 100;
const EXP_PER_FEED: i64 = 10;
const FEED_ENERGY: i64 = 100;
const ENERGY_CAP: i64 = 2000;
const PENDING_CAP: i64 = 20000;
const HUNGER_PER_HOUR: i64 = 8;

/// 返还比例（千分）：等级越高越接近 95%，**恒 < 1000**（回收口硬底线）
fn yield_permille(level: i32) -> i64 {
    (800 + i64::from(level.clamp(1, MAX_LEVEL) - 1) * 15).min(950)
}

fn level_of(exp: i64) -> i32 {
    (1 + exp / EXP_PER_LEVEL).clamp(1, i64::from(MAX_LEVEL)) as i32
}

struct Pet {
    species: String,
    name: String,
    level: i32,
    exp: i64,
    hunger: i32,
    energy: i64,
    pending: i64,
}

/// 读 + 结算到此刻；不存在则创建默认宠物。**自己开事务**（读数/领取路径用它）。
/// 传 `claim_idem` 时把 pending 在同一事务内入账（返回入账额）。
async fn tick(
    db: &sqlx::PgPool,
    uid: i64,
    digest_per_hour: i64,
    claim_idem: Option<&str>,
) -> DomainResult<(Pet, i64)> {
    let mut tx = db.begin().await.map_err(dberr)?;
    let out = tick_tx(&mut tx, uid, digest_per_hour, claim_idem).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(out)
}

/// 事务内版本：投喂要把「扣款 + 推进状态」并进同一笔事务时才用得上 ——
/// 分两次写的话，第二次失败就是「钱扣了、宠物一点没喂到」。
async fn tick_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    digest_per_hour: i64,
    claim_idem: Option<&str>,
) -> DomainResult<(Pet, i64)> {
    let row: Option<(String, String, i32, i64, i32, i64, i64, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT species, name, level, exp, hunger, energy, pending, \
                    last_tick \
               FROM arcade_pets WHERE user_id = $1 FOR UPDATE",
        )
        .bind(uid)
        .fetch_optional(&mut **tx)
        .await
        .map_err(dberr)?;
    let (species, name, level, exp, hunger, energy, pending, last_tick) =
        match row {
            Some(r) => r,
            None => {
                sqlx::query(
                    "INSERT INTO arcade_pets (user_id) VALUES ($1) \
                     ON CONFLICT (user_id) DO NOTHING",
                )
                .bind(uid)
                .execute(&mut **tx)
                .await
                .map_err(dberr)?;
                (
                    "slime".to_string(),
                    String::new(),
                    1,
                    0,
                    100,
                    0,
                    0,
                    Utc::now(),
                )
            }
        };
    let now = Utc::now();
    let dt_ms = (now - last_tick).num_milliseconds().max(0);
    let digestible = ((dt_ms * digest_per_hour) / 3_600_000).clamp(0, energy);
    let gain = digestible * yield_permille(level) / 1000;
    let mut new_pending = (pending + gain).min(PENDING_CAP);
    let new_energy = energy - digestible;
    let new_hunger = (i64::from(hunger) - (dt_ms * HUNGER_PER_HOUR) / 3_600_000)
        .max(0) as i32;
    let mut earned = 0i64;
    if let Some(idem) = claim_idem {
        if new_pending > 0 {
            let out = earn_spark_tx(tx, uid, new_pending, "game", idem).await?;
            // Replayed = 这枚幂等键已入过账：不再重复付，但 pending 仍要清零
            if !matches!(out, SpendOutcome::Replayed) {
                earned = new_pending;
            }
            new_pending = 0;
        }
    }
    sqlx::query(
        "UPDATE arcade_pets SET energy = $2, pending = $3, hunger = $4, \
                last_tick = now() WHERE user_id = $1",
    )
    .bind(uid)
    .bind(new_energy)
    .bind(new_pending)
    .bind(new_hunger)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok((
        Pet {
            species,
            name,
            level,
            exp,
            hunger: new_hunger,
            energy: new_energy,
            pending: new_pending,
        },
        earned,
    ))
}

fn status_json(
    p: &Pet,
    feed_cost: i64,
    digest_per_hour: i64,
) -> serde_json::Value {
    let need = i64::from(p.level) * EXP_PER_LEVEL;
    serde_json::json!({
        "species": p.species,
        "name": p.name,
        "level": p.level,
        "exp": p.exp,
        "exp_to_next": (need - p.exp).max(0),
        "hunger": p.hunger,
        "energy": p.energy,
        "pending": p.pending,
        "yield_permille": yield_permille(p.level),
        "feed_cost": feed_cost,
        "digest_per_hour": digest_per_hour,
        "max_level": MAX_LEVEL,
    })
}

/// 两只设置键（喂价 / 消化速度），缺省即可玩，站长可调。
async fn tune(state: &web::Data<std::sync::Arc<AppState>>) -> (i64, i64) {
    let feed = eco_i64(state, "pet_feed_cost", 100).await.clamp(1, 100_000);
    let dig = eco_i64(state, "pet_digest_per_hour", 300)
        .await
        .clamp(1, 100_000);
    (feed, dig)
}

#[get("/games/pet")]
pub(super) async fn pet_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (feed, dig) = tune(&state).await;
    let (p, _) = tick(&state.repo.db, auth.id, dig, None).await?;
    let coupons: i64 = sqlx::query_scalar(
        "SELECT COALESCE(food_coupons, 0)::bigint FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let mut st = status_json(&p, feed, dig);
    st["food_coupons"] = serde_json::json!(coupons);
    Ok(ok(st))
}

#[derive(Deserialize)]
struct PetActionReq {
    #[serde(default)]
    idempotency_key: Option<String>,
    /// 用口粮券抵扣本次投喂（仅投喂读取；行为联动消耗出口）
    #[serde(default)]
    use_coupon: bool,
}

/// 投喂：扣喂价（或 1 张口粮券抵扣）→ 加能量/经验、满饥饿、可能升级。
#[post("/games/pet/feed")]
pub(super) async fn pet_feed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<PetActionReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 限次（2026-10 审计 P2）：宠物投喂此前不限次，是唯一扣魔力的
    // 无限互动；与农场同档慢节奏（pet_max_feeds_per_hour，缺省 30/时）
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Pet).await?;
    let (feed, dig) = tune(&state).await;
    let (client, use_coupon) = match body {
        Some(b) => (b.idempotency_key.clone(), b.use_coupon),
        None => (None, false),
    };
    let idem = idem_key("pet-feed", auth.id, &client);
    // 扣款（或扣券）与喂食**同一事务**：分两次写的话，第二次失败就是
    // 「钱扣了、没喂到」。
    let mut tx = db.begin().await.map_err(dberr)?;
    let (spent, used_coupon) =
        pay_feed_tx(&mut tx, auth.id, &idem, use_coupon, feed).await?;
    let (p, _) = tick_tx(&mut tx, auth.id, dig, None).await?;
    let exp = p.exp + EXP_PER_FEED;
    let level = level_of(exp);
    let energy = (p.energy + FEED_ENERGY).min(ENERGY_CAP);
    sqlx::query(
        "UPDATE arcade_pets SET energy = $2, exp = $3, level = $4, \
                hunger = 100, last_feed_at = now() WHERE user_id = $1",
    )
    .bind(auth.id)
    .bind(energy)
    .bind(exp)
    .bind(level)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    let after = Pet {
        species: p.species,
        name: p.name,
        level,
        exp,
        hunger: 100,
        energy,
        pending: p.pending,
    };
    state.repo.audit(Some(auth.id), "game.pet.feed", None).await;
    let coupons_left: i64 = sqlx::query_scalar(
        "SELECT COALESCE(food_coupons, 0)::bigint FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "ok": true,
        "spent": spent,
        "used_coupon": used_coupon,
        "food_coupons": coupons_left,
        "status": status_json(&after, feed, dig),
    })))
}

/// 领取：把此刻已消化的 pending 入账（与清零同一事务，防并发/重放双领）。
#[post("/games/pet/claim")]
pub(super) async fn pet_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<PetActionReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (feed, dig) = tune(&state).await;
    let client = body.and_then(|b| b.idempotency_key.clone());
    let idem = idem_key("pet-claim", auth.id, &client);
    let (p, earned) = tick(&state.repo.db, auth.id, dig, Some(&idem)).await?;
    state
        .repo
        .audit(Some(auth.id), "game.pet.claim", None)
        .await;
    Ok(ok(serde_json::json!({
        "earned": earned,
        "status": status_json(&p, feed, dig),
    })))
}

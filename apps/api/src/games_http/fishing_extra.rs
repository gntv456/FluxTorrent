//! 钓鱼生态件：渔汛（周末活动塘）/ 鱼竿升级（sink）/ 渔获图鉴。
//!
//! 渔汛：站点时区（UTC+8）周六/日的「周末窗口」内，对本周做种达标者开放
//! `fishing_event` 活动塘（EV 0.93，比标准塘 0.858 更肥）。门槛沿用行为
//! 联动口径 —— 周做种小时来自 `seeding_reward` 流水（与 linkage.rs 同口径），
//! 阈值 `games_fishing_event_seed_hours`。
//!
//! 鱼竿：花魔力升级（sink），每级加起竿窗口 —— **不改概率**（改概率的
//! luck 类按改版方案 §2.4 属二期）。窗口变宽只提高手速触达率，名义 EV 仍 < 1。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{Datelike, Weekday};
use sqlx::PgPool;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::eco_i64;
use super::pool::{dberr, load_pool};

const ROD_MAX: i32 = 5;
/// 升级费用（策划稿 §2.2：300/600/1000/1500），下标 = 当前等级
const ROD_COSTS: [i64; 4] = [300, 600, 1000, 1500];

/// 本周做种小时：seeding_reward 流水按小时幂等键去重计数（linkage 同口径）
pub(super) async fn weekly_seed_hours(
    db: &PgPool,
    uid: i64,
) -> DomainResult<i64> {
    let v: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT split_part(l.idempotency_key, ':', 3))::bigint \
           FROM spark_ledger l \
          WHERE l.kind = 'seeding_reward' AND l.user_id = $1 \
            AND l.created_at >= date_trunc('week', now() AT TIME ZONE 'UTC') \
              + interval '8 hours'",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(v)
}

/// 渔汛四态：(窗口开启, 本周解锁, 周做种小时, 阈值)。
pub(super) async fn event_state(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<(bool, bool, i64, i64)> {
    let enabled = eco_i64(state, "fishing_event_enabled", 1).await == 1;
    let wd = (chrono::Utc::now() + chrono::Duration::hours(8)).weekday();
    let in_window = enabled && matches!(wd, Weekday::Sat | Weekday::Sun);
    let hours = weekly_seed_hours(&state.repo.db, uid).await?;
    let threshold = eco_i64(state, "games_fishing_event_seed_hours", 20).await;
    Ok((in_window, hours >= threshold, hours, threshold))
}

async fn rod_level(db: &PgPool, uid: i64) -> DomainResult<i32> {
    Ok(sqlx::query_scalar(
        "SELECT level FROM arcade_fishing_rods WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_optional(db)
    .await
    .map_err(dberr)?
    .unwrap_or(1))
}

fn rod_json(level: i32, bonus_per: i64) -> serde_json::Value {
    serde_json::json!({
        "level": level, "max": ROD_MAX,
        "window_bonus_ms": bonus_per * i64::from(level - 1),
        "next_cost": ROD_COSTS.get((level - 1) as usize).copied().unwrap_or(0),
    })
}

#[get("/games/fishing/rod")]
pub(super) async fn fishing_rod(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let level = rod_level(&state.repo.db, auth.id).await?;
    let bonus = eco_i64(&state, "fishing_rod_window_bonus_ms", 150)
        .await
        .clamp(0, 500);
    Ok(ok(rod_json(level, bonus)))
}

/// 升级鱼竿：纯 sink（魔力 → 更宽的起竿窗口 + 竿身外观），不碰概率。
/// 幂等键由服务端按「目标等级」定死：同一次升级重放不重复扣、也不重复 +1。
#[post("/games/fishing/rod/upgrade")]
pub(super) async fn fishing_rod_upgrade(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    sqlx::query(
        "INSERT INTO arcade_fishing_rods (user_id, level) VALUES ($1, 1) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(auth.id)
    .execute(db)
    .await
    .map_err(dberr)?;
    let level = rod_level(db, auth.id).await?;
    if level >= ROD_MAX {
        return Err(DomainError::Validation("鱼竿已满级".into()));
    }
    let cost = ROD_COSTS[(level - 1) as usize];
    let idem = format!("fish-rod:{}:to{}", auth.id, level + 1);
    let mut tx = db.begin().await.map_err(dberr)?;
    if !matches!(
        spend_spark_tx(&mut tx, auth.id, cost, "game", &idem, "fishing_rod", 0)
            .await?,
        SpendOutcome::Spent
    ) {
        // 重放：这次升级早已受理，返回升级后的现状即可
        tx.commit().await.map_err(dberr)?;
        let after = rod_level(db, auth.id).await?;
        let bonus = eco_i64(&state, "fishing_rod_window_bonus_ms", 150)
            .await
            .clamp(0, 500);
        return Ok(ok(rod_json(after, bonus)));
    }
    sqlx::query(
        "UPDATE arcade_fishing_rods SET level = level + 1, upgraded_at = now() \
          WHERE user_id = $1",
    )
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "game.fishing.rod", None)
        .await;
    let bonus = eco_i64(&state, "fishing_rod_window_bonus_ms", 150)
        .await
        .clamp(0, 500);
    let mut out = rod_json(level + 1, bonus);
    out["spent"] = serde_json::json!(cost);
    Ok(ok(out))
}

/// 渔获图鉴：两塘（标准 + 渔汛）的档位即鱼种，捕获数来自对局档位流水。
#[get("/games/fishing/collection")]
pub(super) async fn fishing_collection(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let caught: Vec<(String, i64)> = sqlx::query_as(
        "SELECT prize, count(*)::bigint FROM arcade_pool_rounds \
          WHERE user_id = $1 AND game IN ('fishing', 'fishing_event') \
          GROUP BY prize",
    )
    .bind(auth.id)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let mut fishes: Vec<serde_json::Value> = Vec::new();
    for (game, event) in [("fishing", false), ("fishing_event", true)] {
        let pool = load_pool(db, game).await?;
        for (i, e) in pool.entries.iter().enumerate() {
            let rar = pool.meta.get(i).map(|m| m.0).unwrap_or(1);
            let count = caught
                .iter()
                .find(|(p, _)| *p == e.label)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            fishes.push(serde_json::json!({
                "label": e.label, "rarity": rar, "event": event,
                "count": count,
                "mult": e.mult_permille() as f64 / 1000.0,
            }));
        }
    }
    let total = fishes.len() as i64;
    let got = fishes
        .iter()
        .filter(|f| f["count"].as_i64().unwrap_or(0) > 0)
        .count() as i64;
    Ok(ok(serde_json::json!({
        "got": got, "total": total, "fishes": fishes,
    })))
}

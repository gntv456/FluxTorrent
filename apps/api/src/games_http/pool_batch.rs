//! 池抽类玩法的**批量连抽**（从 pool_games.rs 拆出：撞 300 行上限）。
//!
//! 单局纪律（限次/幂等/事务）都在 pool_games.rs 的 `round_in_tx`；
//! 这里的职责只有一件事：把 N 局装进**一个事务**，限次按 N 计数、
//! 余额预截断、整组同生共死，外加「整组幂等回放」。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{check_minclass, eco_i64};
use super::pool::{dberr, load_pool};
use super::pool_games::{round_in_tx, MAX_BATCH};

/// 批量连抽（2026-11-21 收口）：N 局一个事务。
///
/// 旧实现（客户端并发 N 个单抽）的缺口：限次/余额是**逐发**判定，
/// 极端并发下余额只够 3 局时百连会截断成 3+N 发 4xx——客户端把
/// 「已受理」与「已拒绝」混在一组结果里，账面对不平。这里把
/// 「限次计数 → 余额预截断 → 逐局扣票/结算/回放」放进同一事务：
/// 要么整组全成功，要么整组回滚，没有半截结果。
///
/// 限次口径：批量按 N 计数（与单抽同一条小时窗）。先用 `limit_used`
/// 读余量做精确截断（余 12 局时百连=12 局），再 Redis INCR N——INCR
/// 失败仍 fail-close 拒绝（P2-7 同款纪律），不会只扣不抽。
async fn pool_batch(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    game: &str,
    count: i64,
    client_idem: Option<String>,
) -> DomainResult<serde_json::Value> {
    let pool = load_pool(&state.repo.db, game).await?;
    let ticket = pool.ticket;
    let max_bet = eco_i64(state, "games_max_bet", games::MAX_BET).await;
    if ticket > max_bet {
        return Err(DomainError::Validation(format!(
            "票价 {ticket} 超过单次上限 {max_bet}，当前配置下无法开抽"
        )));
    }
    let count = count.clamp(1, MAX_BATCH);
    // 余额预截断（先于限次 INCR：买不起的局数不去占额度）
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(uid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(dberr)?;
    let afford = if ticket > 0 { balance / ticket } else { 0 };
    // 限次精确截断：余量 = 上限 - 已用（读不到已用=本小时还没玩过=满额）。
    // 批量 INCR N 原子占用；超限则按实际余量回退，不发注定 429 的整组请求。
    let limit =
        eco_i64(state, "games_max_plays_per_hour", games::MAX_PLAYS_PER_HOUR)
            .await;
    let used = super::helpers::limit_used(state, uid, false)
        .await
        .unwrap_or(0);
    let by_limit = (limit - used).max(0);
    let n = count.min(afford).min(by_limit);
    if n <= 0 {
        if afford <= 0 {
            return Err(DomainError::InsufficientSpark);
        }
        return Err(DomainError::RateLimited);
    }
    // 原子占用 n 次额度（fail-close：Redis 不可用整组拒绝，不冒超抽）
    {
        use redis::AsyncCommands;
        let key = super::helpers::rate_key(uid, false);
        let mut conn = state.redis.clone();
        let after: i64 = conn.incr(&key, n).await.map_err(|_| {
            DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
        })?;
        let ttl: i64 = conn.pttl(&key).await.unwrap_or(-1);
        if ttl < 0 {
            let _ = conn.expire::<_, i64>(&key, 3600).await;
        }
        if after > limit {
            // INCR 后超限：把多占的退回去（扣回 n），按余量重截。
            let over = after - limit;
            let keep = n - over;
            let _: i64 = conn.incr(&key, -over).await.unwrap_or(after);
            if keep <= 0 {
                return Err(DomainError::RateLimited);
            }
            // keep 与预截断一致（同一窗口内无人插队时），按 keep 抽
            return pool_batch_run(state, uid, game, &pool, keep, &client_idem)
                .await;
        }
    }
    pool_batch_run(state, uid, game, &pool, n, &client_idem).await
}

/// 批量执行段：n 局同一事务，逐局 扣票→抽档→结算→回放。
/// 结果按局序返回；任一局失败整组回滚（没有半截账）。
async fn pool_batch_run(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    game: &str,
    pool: &super::pool::Pool,
    n: i64,
    client_idem: &Option<String>,
) -> DomainResult<serde_json::Value> {
    let base = batch_base(client_idem);
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(n as usize);
    let mut total_net: i64 = 0;
    for i in 0..n {
        let idem = batch_idem(game, uid, &base, i);
        let r = round_in_tx(&mut tx, uid, game, pool, &idem).await?;
        // 整组幂等回放：响应丢失后客户端用同一组键重试，首局命中
        // 「已受理」说明整组都已结算过——逐局从回放表取原结果，
        // 不重抽不重扣，也不能报「流水冲突」让玩家重试无门。
        if r.is_none() {
            let _ = tx.rollback().await;
            return replay_batch(&state.repo.db, game, uid, &base, n).await;
        }
        let (mult, rarity, idx, label, kind, fell_back, replay, spark, value) =
            r.unwrap();
        rows.push(serde_json::json!({
            "index": idx,
            "prize": label,
            "rarity": rarity,
            "kind": kind,
            "fell_back": fell_back,
            "replay": replay,
            "payout": spark,
            "value": value,
            "ticket": pool.ticket,
            "net": value - pool.ticket,
            // 魔力档的语言无关读数：客户端用 ×N 组合展示，DB 中文串只作兜底
            "mult": mult,
        }));
        total_net += value - pool.ticket;
    }
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(uid), &format!("game.{game}.batch"), None)
        .await;
    Ok(serde_json::json!({
        "rows": rows,
        "total_net": total_net,
        "count": rows.len(),
    }))
}

/// 组内派生键：`game-<game>:<uid>:<base>-g<i>`（g{i} 与单抽键同命名空间）。
fn batch_idem(game: &str, uid: i64, base: &str, i: i64) -> String {
    format!("game-{game}:{uid}:{base}-g{i}")
}

/// 组基键：客户端自报的幂等键（同单抽口径校验），缺省时随机——
/// 随机基键意味着这组不可回放，但至少不会撞别人的键。
fn batch_base(client_idem: &Option<String>) -> String {
    client_idem
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map_or_else(|| uuid::Uuid::new_v4().to_string(), |s| s.to_owned())
}

/// 整组回放：按派生键 `base-g{i}` 逐局读回放表。组键是客户端自报的，
/// 若首局已受理但后续局缺失（同组键并发首发的极端交错），缺失局按
/// 原局数补不了——原样返回已有局数并注明，客户端按 count 对账。
/// mult 读数与单抽回放同一把尺（已落库 value/ticket，不按当前池序回算）。
async fn replay_batch(
    db: &sqlx::PgPool,
    game: &str,
    uid: i64,
    base: &str,
    n: i64,
) -> DomainResult<serde_json::Value> {
    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(n as usize);
    let mut total_net: i64 = 0;
    for i in 0..n {
        let idem = batch_idem(game, uid, base, i);
        let Some(row) =
            super::pool_games::replay_one(db, &idem, Some(uid), Some(game))
                .await
                .ok()
        else {
            break;
        };
        total_net += row["net"].as_i64().unwrap_or(0);
        rows.push(row);
    }
    Ok(serde_json::json!({
        "rows": rows,
        "total_net": total_net,
        "count": rows.len(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchReq {
    /// 局数（1..=MAX_BATCH，越界钳制不发 4xx）
    count: i64,
    /// 组幂等键：响应丢失后同一键重试，服务端按派生键（base-g{i}）整组回放
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 大转盘批量连抽：N 局一个事务（限次/余额/扣票/结算/回放同生共死）。
#[post("/games/wheel/batch")]
pub(super) async fn wheel_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: crate::http::OptJson<BatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "wheel", auth.class_id).await?;
    let (count, client_idem) = body
        .map(|b| (b.count, b.idempotency_key))
        .unwrap_or((1, None));
    Ok(ok(
        pool_batch(&state, auth.id, "wheel", count, client_idem).await?
    ))
}

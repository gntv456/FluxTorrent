//! 池抽类玩法：九宫格 / 扭蛋机 / 大转盘（从 casino.rs 拆出：撞 300 行上限）。
//!
//! 三者**同构**（票价 + 加权池抽档），只差 `game`（决定池、流水 ref_type、审计名）。
//! 三处各写一遍必然漂移，故只留 `pool_round` 一份实现，三个 handler 都是薄壳。
//! 结算细节（魔力位入账 / 物品位发放 / 回落折魔力）复用 casino.rs 的 `settle`。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::casino::{award_kind, settle_tx};
use super::helpers::{check_rate, eco_i64, idem_key};
use super::pool::{dberr, load_pool};

#[derive(Deserialize)]
struct DrawReq {
    /// 客户端幂等键（审计 P2-8）
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 池抽类玩法通用一局：扣票 → 抽档 → 结算。
async fn pool_round(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    game: &str,
    client_idem: Option<String>,
) -> DomainResult<serde_json::Value> {
    let pool = load_pool(&state.repo.db, game).await?;
    let ticket = pool.ticket;
    // 风控一致性：票价同样受「单次下注上限」约束（旧实现绕过 games_max_bet，
    // 站长把上限调到 100 以下时仍能抽走 100）。先校验再计数，避免白耗次数。
    let max_bet = eco_i64(state, "games_max_bet", games::MAX_BET).await;
    if ticket > max_bet {
        return Err(DomainError::Validation(format!(
            "票价 {ticket} 超过单次上限 {max_bet}，当前配置下无法开抽"
        )));
    }
    check_rate(state, &state.redis, uid).await?;

    let idem = idem_key(game, uid, &client_idem);
    // 抽档是纯计算，先定死；再开一笔事务把「扣票 + 结算」一起提交 ——
    // 分两次写的话，结算失败就是「票价扣了、奖没开」，玩家白亏一注。
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("奖池不可抽样".into()))?;
    // 魔力位直接入账；物品位走发放账（库存/上限/幂等同一事务判），
    // 发不出去时按 FALLBACK_MULT 折魔力 —— 这条回落已被 EV 计入，不是额外成本。
    let win_idem = format!("game-{game}-win:{idem}");
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    // 幂等（同 scratch）：重放不重开，避免「首局没中 + 重放中了」白赚
    if !matches!(
        spend_spark_tx(&mut tx, uid, ticket, "game", &idem, game, 0).await?,
        SpendOutcome::Spent
    ) {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    let (spark, value, fell_back) = settle_tx(
        &mut tx,
        uid,
        ticket,
        &draw,
        game,
        &win_idem,
        super::casino::meta_rarity(&pool.meta, draw.index),
    )
    .await?;
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(uid), &format!("game.{game}"), None)
        .await;
    Ok(serde_json::json!({
        "index": draw.index,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "payout": spark,
        "value": value,
        "ticket": ticket,
        "net": value - ticket,
    }))
}

#[post("/games/jgg")]
pub(super) async fn jgg(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<DrawReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "jgg", client_idem).await?))
}

/// 扭蛋机：与九宫格同构，仅池键不同（capsule）。
#[post("/games/capsule")]
pub(super) async fn capsule(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<DrawReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "capsule", client_idem).await?))
}

/// 大转盘：与九宫格同构，仅池键不同（wheel）。
#[post("/games/wheel")]
pub(super) async fn wheel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<DrawReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "wheel", client_idem).await?))
}

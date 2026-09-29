//! 刮刮乐/猜大小/九宫格（M24 博彩三件套，EV<1 回收型）。
//! 从 games_http.rs 按域拆出；限次/赔率助手在 helpers.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, Guess};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{
    bigsmall_mult_permille, check_bet, check_rate, eco_i64, idem_key,
    scratch_odds, BetReq,
};
use super::pool::{grant_item, load_pool, GrantOutcome};

#[post("/games/scratch")]
pub(super) async fn scratch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    check_rate(&state, &state.redis, auth.id).await?;

    let idem = idem_key("scratch", auth.id, &body.idempotency_key);
    // 幂等：同一键重复提交（网络重试/双击）不重复扣款，也**不重开一次奖** ——
    // 否则「首局未中奖 + 重放中奖」= 白赚，是必须堵住的印钞口。
    if !matches!(
        spend_spark(
            &state.repo.db,
            auth.id,
            body.bet,
            "game",
            &idem,
            "scratch",
            0
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    // 概率档位读 0109 设置键（缺省回落 45/30/15/8/2）
    let odds = scratch_odds(&state).await?;
    let outcome = games::scratch_play_with(body.bet, &odds);
    if outcome.payout > 0 {
        let win_idem = format!("game-scratch-win:{}", idem);
        earn_spark(&state.repo.db, auth.id, outcome.payout, "game", &win_idem)
            .await?;
    }
    Ok(ok(serde_json::json!({
        "multiplier": outcome.multiplier,
        "payout": outcome.payout,
        "bet": body.bet,
        "net": outcome.payout - body.bet,
    })))
}

#[derive(Deserialize)]
struct GuessReq {
    bet: i64,
    guess: String, // "small" | "big"
    /// 客户端幂等键（审计 P2-8）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/games/bigsmall")]
pub(super) async fn guess_bigsmall(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GuessReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    let guess = match body.guess.as_str() {
        "small" => Guess::Small,
        "big" => Guess::Big,
        _ => {
            return Err(DomainError::Validation(
                "guess 仅支持 small/big".into(),
            ))
        }
    };
    check_rate(&state, &state.redis, auth.id).await?;

    let idem = idem_key("bs", auth.id, &body.idempotency_key);
    // 幂等（同 scratch）：重放不重开，避免「首局没中 + 重放中了」白赚
    if !matches!(
        spend_spark(
            &state.repo.db,
            auth.id,
            body.bet,
            "game",
            &idem,
            "bigsmall",
            0
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    let outcome = games::guess_play_with(
        body.bet,
        guess,
        bigsmall_mult_permille(&state).await,
    );
    if outcome.payout > 0 {
        let win_idem = format!("game-bs-win:{}", idem);
        earn_spark(&state.repo.db, auth.id, outcome.payout, "game", &win_idem)
            .await?;
    }
    Ok(ok(serde_json::json!({
        "number": outcome.number,
        "player_win": outcome.player_win,
        "payout": outcome.payout,
        "bet": body.bet,
        "net": if (50..=51).contains(&outcome.number) { 0 } else { outcome.payout - body.bet },
        "tie": (50..=51).contains(&outcome.number),
    })))
}

// ============ 九宫格抽奖（jgg 口径） ============

#[derive(Deserialize)]
struct JggReq {
    /// 客户端幂等键（审计 P2-8）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/games/jgg")]
pub(super) async fn jgg(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<JggReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 风控一致性：票价同样受「单次下注上限」约束（旧实现绕过 games_max_bet，
    // 站长把上限调到 100 以下时仍能抽走 100）。先校验再计数，避免白耗次数。
    let pool = load_pool(&state.repo.db, "jgg").await?;
    let ticket = pool.ticket;
    let max_bet = eco_i64(&state, "games_max_bet", games::MAX_BET).await;
    if ticket > max_bet {
        return Err(DomainError::Validation(format!(
            "票价 {ticket} 超过单次上限 {max_bet}，当前配置下无法开抽"
        )));
    }
    check_rate(&state, &state.redis, auth.id).await?;

    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    let idem = idem_key("jgg", auth.id, &client_idem);
    // 幂等（同 scratch）：重放不重开
    if !matches!(
        spend_spark(&state.repo.db, auth.id, ticket, "game", &idem, "jgg", 0)
            .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("奖池不可抽样".into()))?;
    // 魔力位直接入账；物品位走发放账（库存 / 每人上限 / 幂等同一事务判），
    // 发不出去时按 FALLBACK_MULT 折魔力 —— 这条回落路径已被 EV 计入，不是额外成本。
    let win_idem = format!("game-jgg-win:{}", idem);
    let (spark, value, fell_back) = match &draw.prize.kind {
        games::EntryKind::Magic { multiples } => {
            (ticket * multiples, ticket * multiples, None)
        }
        games::EntryKind::Item {
            item_key,
            qty,
            anchor,
        } => match grant_item(
            &state.repo.db,
            auth.id,
            item_key,
            *qty,
            "jgg",
            &win_idem,
        )
        .await?
        {
            GrantOutcome::Granted => (0, anchor * i64::from(*qty), None),
            GrantOutcome::FellBack(why) => {
                let p = ticket * games::FALLBACK_MULT;
                (p, p, Some(why))
            }
        },
    };
    if spark > 0 {
        earn_spark(&state.repo.db, auth.id, spark, "game", &win_idem).await?;
    }
    // 公示口径：物品 / 回落 / 魔力 三种结果在前台必须可区分
    let award_kind = match (&draw.prize.kind, fell_back) {
        (_, Some(_)) => "fallback",
        (games::EntryKind::Magic { .. }, _) => "magic",
        (games::EntryKind::Item { .. }, _) => "item",
    };
    state.repo.audit(Some(auth.id), "game.jgg", None).await;
    Ok(ok(serde_json::json!({
        "index": draw.index,
        "prize": draw.prize.label,
        "kind": award_kind,
        "fell_back": fell_back,
        "payout": spark,
        "value": value,
        "ticket": ticket,
        "net": value - ticket,
    })))
}

// ============ 农场 ============

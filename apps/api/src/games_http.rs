//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小 + 每小时限次）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, validate_bet, Guess, MAX_PLAYS_PER_HOUR};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_games(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(games_overview)
        .service(scratch)
        .service(guess_bigsmall)
}

#[get("/games")]
async fn games_overview() -> impl Responder {
    ok(serde_json::json!({
        "scratch": { "name": "刮刮乐", "max_bet": games::MAX_BET,
            "prizes": ["0.5x (30%)", "1x (15%)", "2x (8%)", "10x (2%)"] },
        "bigsmall": { "name": "猜大小", "max_bet": games::MAX_BET,
            "rule": "1-49 小 · 52-100 大 · 50/51 平局返本 · 猜中 2x" },
        "rate_limit": format!("每人每小时 {MAX_PLAYS_PER_HOUR} 次"),
    }))
}

/// 每小时限次（Redis INCR + EXPIRE）
async fn check_rate(redis: &redis::aio::ConnectionManager, user_id: i64) -> DomainResult<()> {
    use redis::AsyncCommands;
    let key = format!("rl:games:{user_id}");
    let mut conn = redis.clone();
    let n: i64 = conn.incr(&key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = conn.expire(&key, 3600).await.unwrap_or(());
    }
    if n > MAX_PLAYS_PER_HOUR {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

#[derive(Deserialize)]
struct BetReq {
    bet: i64,
}

#[post("/games/scratch")]
async fn scratch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    validate_bet(body.bet).map_err(DomainError::Validation)?;
    check_rate(&state.redis, auth.id).await?;

    let idem = format!("game-scratch:{}:{}", auth.id, Uuid::new_v4());
    spend_spark(
        &state.repo.db,
        auth.id,
        body.bet,
        "game",
        &idem,
        "scratch",
        0,
    )
    .await?;
    let outcome = games::scratch_play(body.bet);
    if outcome.payout > 0 {
        let win_idem = format!("game-scratch-win:{}", idem);
        earn_spark(&state.repo.db, auth.id, outcome.payout, "game", &win_idem).await?;
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
}

#[post("/games/bigsmall")]
async fn guess_bigsmall(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GuessReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    validate_bet(body.bet).map_err(DomainError::Validation)?;
    let guess = match body.guess.as_str() {
        "small" => Guess::Small,
        "big" => Guess::Big,
        _ => return Err(DomainError::Validation("guess 仅支持 small/big".into())),
    };
    check_rate(&state.redis, auth.id).await?;

    let idem = format!("game-bs:{}:{}", auth.id, Uuid::new_v4());
    spend_spark(
        &state.repo.db,
        auth.id,
        body.bet,
        "game",
        &idem,
        "bigsmall",
        0,
    )
    .await?;
    let outcome = games::guess_play(body.bet, guess);
    if outcome.payout > 0 {
        let win_idem = format!("game-bs-win:{}", idem);
        earn_spark(&state.repo.db, auth.id, outcome.payout, "game", &win_idem).await?;
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

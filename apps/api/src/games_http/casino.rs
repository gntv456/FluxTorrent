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

use super::helpers::{check_bet, check_rate, eco_i64, idem_key, BetReq};
use super::pool::{grant_item, load_pool, load_table, GrantOutcome};

/// 一档的结算：魔力位按 `unit × 千分倍率 / 1000` 派彩，物品位走发放账。
///
/// `unit` 对九宫格是票价、对刮刮乐是玩家这注的金额 —— 两侧口径本来就一样，
/// 写两遍的话「物品发不出去怎么算」这类规则迟早会在其中一侧偷偷改掉。
/// 发不出去时按 `FALLBACK_MULT` 折魔力，这条回落已被 EV 计入，不是额外成本。
async fn settle(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    unit: i64,
    draw: &games::Draw,
    game: &str,
    win_idem: &str,
) -> DomainResult<(i64, i64, Option<&'static str>)> {
    let (spark, value, fell_back) = match &draw.prize.kind {
        games::EntryKind::Magic { mult_permille } => {
            let p = unit.saturating_mul(*mult_permille) / games::MULT_UNIT;
            (p, p, None)
        }
        games::EntryKind::Item {
            item_key,
            qty,
            anchor,
        } => match grant_item(
            &state.repo.db,
            uid,
            item_key,
            *qty,
            game,
            "rand",
            win_idem,
        )
        .await?
        {
            GrantOutcome::Granted => {
                (0, anchor.saturating_mul(i64::from(*qty)), None)
            }
            GrantOutcome::FellBack(why) => {
                let p = unit.saturating_mul(games::FALLBACK_MULT);
                (p, p, Some(why))
            }
        },
    };
    if spark > 0 {
        earn_spark(&state.repo.db, uid, spark, "game", win_idem).await?;
    }
    Ok((spark, value, fell_back))
}

/// 中奖结果的类型，公示与前台都要按它区分渲染
fn award_kind(draw: &games::Draw, fell_back: Option<&str>) -> &'static str {
    match (&draw.prize.kind, fell_back) {
        (_, Some(_)) => "fallback",
        (games::EntryKind::Magic { .. }, _) => "magic",
        (games::EntryKind::Item { .. }, _) => "item",
    }
}

#[post("/games/scratch")]
pub(super) async fn scratch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 档位与概率在 arcade_pools(game='scratch')：不合法直接拒服务，
    // 不再从设置键回落缺省表（0248 之前那条静默回落等于「改错一个字就当没事」）
    let pool = load_pool(&state.repo.db, "scratch").await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < pool.ticket {
        return Err(DomainError::Validation(format!(
            "刮刮乐最低注额为 {} 魔力：注额低于票档时，固定折算价的物品位\
             会让小额注的综合返还冲破 1",
            pool.ticket
        )));
    }
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
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("刮刮乐奖池不可抽样".into()))?;
    let win_idem = format!("game-scratch-win:{}", idem);
    let (spark, value, fell_back) =
        settle(&state, auth.id, body.bet, &draw, "scratch", &win_idem).await?;
    state.repo.audit(Some(auth.id), "game.scratch", None).await;
    Ok(ok(serde_json::json!({
        "multiplier": draw.prize.mult_permille() as f64 / 1000.0,
        "payout": spark,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "bet": body.bet,
        "value": value,
        "net": value - body.bet,
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
    // 三区档位来自 arcade_pools(game='bigsmall')：赔率、平局返本、以及
    // 「猜中给一件东西」都是站长可配的，配坏则整桌拒绝服务（load_table 里过关）
    let table = load_table(&state.repo.db, "bigsmall").await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < table.ticket {
        return Err(DomainError::Validation(format!(
            "最低注额为 {} 魔力：注额低于票档时，固定折算价的物品位             会让小额注的综合返还冲破 1",
            table.ticket
        )));
    }
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
    // 机制在代码（1..100 均匀、49/2/49 分区），派彩在表：平局返本也是表里
    // 一条 1000‰ 的魔力位，不再由代码特判 —— 少一处「规则写在两个地方」
    let number = games::roll();
    let side = games::outcome_side(number, guess);
    let draw = games::draw_entry(table.region(side)).ok_or_else(|| {
        DomainError::Validation("猜大小该档区不可抽样".into())
    })?;
    let win_idem = format!("game-bs-win:{}", idem);
    let (spark, value, fell_back) =
        settle(&state, auth.id, body.bet, &draw, "bigsmall", &win_idem).await?;
    state.repo.audit(Some(auth.id), "game.bigsmall", None).await;
    Ok(ok(serde_json::json!({
        "number": number,
        "player_win": side == "win",
        "side": side,
        "payout": spark,
        "value": value,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "bet": body.bet,
        "net": value - body.bet,
        "tie": side == "tie",
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
    let (spark, value, fell_back) =
        settle(&state, auth.id, ticket, &draw, "jgg", &win_idem).await?;
    state.repo.audit(Some(auth.id), "game.jgg", None).await;
    Ok(ok(serde_json::json!({
        "index": draw.index,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "payout": spark,
        "value": value,
        "ticket": ticket,
        "net": value - ticket,
    })))
}

// ============ 农场 ============

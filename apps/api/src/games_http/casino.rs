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

use super::helpers::{check_bet, check_rate, idem_key, BetReq};
use super::pool::{grant_item, load_pool, load_table, GrantOutcome};

/// 一档的结算：魔力位按 `unit × 千分倍率 / 1000` 派彩，物品位走发放账。
///
/// `unit` 对九宫格是票价、对刮刮乐是玩家这注的金额 —— 两侧口径本来就一样，
/// 写两遍的话「物品发不出去怎么算」这类规则迟早会在其中一侧偷偷改掉。
/// 发不出去时按 `FALLBACK_MULT` 折魔力，这条回落已被 EV 计入，不是额外成本。
pub(super) async fn settle(
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
pub(super) fn award_kind(draw: &games::Draw, fell_back: Option<&str>) -> &'static str {
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
    /// 本局挂载的道具 key（可选）：只加权魔力的输赢，永不出物品
    #[serde(default)]
    props: Vec<String>,
}

#[post("/games/bigsmall")]
pub(super) async fn guess_bigsmall(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GuessReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 三区档位来自 arcade_pools(game='bigsmall')：**纯魔力**（物品位在读侧被拒），
    // 配坏则整桌拒绝服务（load_table 里过关）
    let table = load_table(db, "bigsmall").await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < table.ticket {
        return Err(DomainError::Validation(format!(
            "最低注额为 {} 魔力",
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
    // 道具先校验（只读）再扣款：校验失败时不能已经扣了注额
    let props = super::bigsmall_props::pick(db, auth.id, &body.props).await?;
    check_rate(&state, &state.redis, auth.id).await?;

    let idem = idem_key("bs", auth.id, &body.idempotency_key);
    // 幂等（同 scratch）：重放不重开，避免「首局没中 + 重放中了」白赚
    if !matches!(
        spend_spark(db, auth.id, body.bet, "game", &idem, "bigsmall", 0).await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    // 扣道具（幂等键 = 本局 + 道具 key）：重放不重复扣，也不吞掉一件
    for p in &props {
        super::bigsmall_props::consume(
            db,
            auth.id,
            &p.key,
            "bigsmall",
            &format!("game-bs-prop:{idem}:{}", p.key),
        )
        .await?;
    }
    // 机制在代码（1..100 均匀、49/2/49 分区），派彩在表
    let number = games::roll();
    let side = games::outcome_side(number, guess);
    let draw = games::draw_entry(table.region(side)).ok_or_else(|| {
        DomainError::Validation("猜大小该档区不可抽样".into())
    })?;
    let win_idem = format!("game-bs-win:{}", idem);
    let (base_spark, value, fell_back) =
        settle(&state, auth.id, body.bet, &draw, "bigsmall", &win_idem).await?;
    // 道具加权：只改魔力的输赢幅度，不产生任何物品
    let ap = super::bigsmall_props::apply(
        &props,
        body.bet,
        side,
        base_spark,
        draw.prize.mult_permille(),
    );
    if ap.extra > 0 {
        earn_spark(
            db,
            auth.id,
            ap.extra,
            "game",
            &format!("game-bs-prop-win:{idem}"),
        )
        .await?;
    }
    let payout = base_spark + ap.extra;
    state.repo.audit(Some(auth.id), "game.bigsmall", None).await;
    Ok(ok(serde_json::json!({
        "number": number,
        "player_win": side == "win",
        "side": side,
        "payout": payout,
        "base_payout": base_spark,
        "value": value + ap.extra,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "bet": body.bet,
        "net": payout - body.bet,
        "tie": side == "tie",
        "props_applied": props.iter().map(|p| serde_json::json!({
            "key": p.key, "name": p.name, "icon": p.icon,
            "effect": p.effect, "value": p.value,
        })).collect::<Vec<_>>(),
        "shield_refund": ap.shield_refund,
        "effective_mult": ap.effective_mult.map(|m| m as f64 / 1000.0),
    })))
}

// ============ 农场 ============

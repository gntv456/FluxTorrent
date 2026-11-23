//! 猜大小「倒计时房间」HTTP 口：GET 房间状态（倒计时/两侧注额/我的注/上期开奖），
//! POST 本期下注。房间制与旧即时局 `/games/bigsmall` 并存：旧口保留兼容，新口走
//! 期次表（0515）。经济口径不变：魔力记账、等级门、每小时限次、幂等防重放、EV<1。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::Utc;
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, BIGSMALL_WIN_MULT_PERMILLE};
use crate::http::require_auth;
use crate::state::AppState;

use super::bs_room_store::{
    add_side_total, advance_open, insert_bet, last_settled, my_bets,
};
use super::helpers::{
    check_bet, check_minclass, check_rate, eco_i64, idem_key,
};
use super::pool::{dberr, load_table};

/// 读桌（关闸校验）+ 命中侧千分倍率 + 下注窗口秒数。
async fn room_config(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<(super::pool::Table, i64, i64)> {
    let table = load_table(&state.repo.db, "bigsmall").await?;
    let mult = table
        .win
        .first()
        .map(|e| e.mult_permille())
        .filter(|m| *m > 0)
        .unwrap_or(games::BIGSMALL_WIN_MULT_PERMILLE);
    let window = games::window_seconds(
        eco_i64(
            state,
            "bigsmall_round_seconds",
            games::ROUND_SECONDS_DEFAULT,
        )
        .await,
    );
    Ok((table, mult, window))
}

#[get("/games/bigsmall/room")]
pub(super) async fn room_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "bigsmall", auth.class_id).await?;
    let (table, mult, window) = room_config(&state).await?;

    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    // 派彩走抽水后的等效倍率（1710），面板 mult 显示仍回原值（1900）
    let open =
        advance_open(&mut tx, games::raked_mult_permille(mult), window).await?;
    let mine = my_bets(&mut tx, open.id, auth.id).await?;
    let last = last_settled(&mut tx).await?;
    tx.commit().await.map_err(dberr)?;

    let elapsed = Utc::now()
        .signed_duration_since(open.opened_at)
        .num_seconds();
    let remaining = games::remaining_seconds(elapsed, window);
    let my_total: i64 = mine.iter().map(|b| b.amount).sum();

    Ok(ok(serde_json::json!({
        "round": {
            "id": open.id,
            "remaining": remaining,
            "window": window,
            "total_small": open.total_small,
            "total_big": open.total_big,
        },
        "ticket": table.ticket,
        "mult": mult,
        "win_mult": mult as f64 / 1000.0,
        "mine": mine.iter().map(|b| serde_json::json!({
            "side": b.side, "amount": b.amount,
            "payout": b.payout, "net": b.net,
        })).collect::<Vec<_>>(),
        "my_total": my_total,
        "last": last.map(|l| serde_json::json!({
            "dice": l.dice, "sum": l.sum, "region": l.region,
        })),
    })))
}

#[derive(Deserialize)]
// 写口多带键不静默忽略（P-314 一族）：拼错字段不吞注额。
#[serde(deny_unknown_fields)]
struct RoomBetReq {
    bet: i64,
    side: String, // "small" | "big"
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/games/bigsmall/room/bet")]
pub(super) async fn room_bet(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RoomBetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "bigsmall", auth.class_id).await?;
    let (table, mult, window) = room_config(&state).await?;

    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < table.ticket {
        return Err(DomainError::Validation(format!(
            "最低注额为 {} 魔力",
            table.ticket
        )));
    }
    if games::parse_side(&body.side).is_none() {
        return Err(DomainError::Validation("side 仅支持 small/big".into()));
    }
    check_rate(&state, &state.redis, auth.id).await?;

    let idem = idem_key("bs-room", auth.id, &body.idempotency_key);
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    // 先推进到当前 betting 期：过期期在事务内结算（派彩走抽水后等效倍率），
    // 再开新期。
    let open =
        advance_open(&mut tx, games::raked_mult_permille(mult), window).await?;

    let elapsed = Utc::now()
        .signed_duration_since(open.opened_at)
        .num_seconds();
    if elapsed >= window as i64 {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("本期已截止，请等待新一期".into()));
    }

    // 扣注：幂等键撞 = 重放，不重复扣款。
    if !matches!(
        spend_spark_tx(
            &mut tx, auth.id, body.bet, "game", &idem, "bigsmall", open.id
        )
        .await?,
        SpendOutcome::Spent
    ) {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("该注已受理，请勿重复提交".into()));
    }
    insert_bet(&mut tx, open.id, auth.id, &body.side, body.bet, &idem).await?;
    add_side_total(&mut tx, open.id, &body.side, body.bet).await?;
    let mine = my_bets(&mut tx, open.id, auth.id).await?;
    tx.commit().await.map_err(dberr)?;

    state
        .repo
        .audit(Some(auth.id), "game.bigsmall_room", None)
        .await;

    let remaining = games::remaining_seconds(elapsed, window);
    Ok(ok(serde_json::json!({
        "round_id": open.id,
        "side": body.side,
        "amount": body.bet,
        "remaining": remaining,
        "mine": mine.iter().map(|b| serde_json::json!({
            "side": b.side, "amount": b.amount,
            "payout": b.payout, "net": b.net,
        })).collect::<Vec<_>>(),
    })))
}

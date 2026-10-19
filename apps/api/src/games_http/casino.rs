//! 刮刮乐/猜大小/九宫格（M24 博彩三件套，EV<1 回收型）。
//! 从 games_http.rs 按域拆出；限次/赔率助手在 helpers.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, Guess};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{check_bet, check_rate, idem_key, BetReq};
use super::pool::{dberr, grant_item_tx, load_pool, load_table, GrantOutcome};

/// 档位展示元数据 (rarity, image_url) 里取稀有度；越界回落 1（未中奖不发光）
pub(super) fn meta_rarity(meta: &[(i16, String)], i: usize) -> i16 {
    meta.get(i).map(|m| m.0).unwrap_or(1)
}

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
    rarity: i16,
) -> DomainResult<(i64, i64, Option<&'static str>)> {
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let out =
        settle_tx(&mut tx, uid, unit, draw, game, win_idem, rarity).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(out)
}

/// 事务内版本：把「扣注额 + 结算」并进一笔事务时用它 ——
/// 分两次写的话，第二次失败就是「注额扣了、什么都没发生」，玩家白亏一注。
/// 每局顺带落一行 arcade_pool_rounds（稀有播报 / 图鉴的事实源）。
pub(super) async fn settle_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    unit: i64,
    draw: &games::Draw,
    game: &str,
    win_idem: &str,
    rarity: i16,
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
        } => {
            match grant_item_tx(tx, uid, item_key, *qty, game, "rand", win_idem)
                .await?
            {
                GrantOutcome::Granted => {
                    (0, anchor.saturating_mul(i64::from(*qty)), None)
                }
                GrantOutcome::FellBack(why) => {
                    let p = unit.saturating_mul(games::FALLBACK_MULT);
                    (p, p, Some(why))
                }
            }
        }
    };
    if spark > 0 {
        // 入账结果必须消费（P-347）：调用链已用 spend_spark_tx 的 Spent 闸住
        // 重放，正常路径必然 Spent；一旦回到 Replayed 说明这个幂等键被别的口
        // 占用，继续往下写 arcade_pool_rounds 就成了「玩家看到中奖、钱没入账」
        // ——宁可整笔回滚（流水冲突，请重试）也不冒这个险。
        if !matches!(
            earn_spark_tx(tx, uid, spark, "game", win_idem).await?,
            SpendOutcome::Spent
        ) {
            return Err(DomainError::LedgerConflict);
        }
    }
    sqlx::query(
        "INSERT INTO arcade_pool_rounds (user_id, game, entry_index, \
         rarity, prize, spark, value) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(uid)
    .bind(game)
    .bind(draw.index as i32)
    .bind(rarity)
    .bind(&draw.prize.label)
    .bind(spark)
    .bind(value)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok((spark, value, fell_back))
}

/// 中奖结果的类型，公示与前台都要按它区分渲染
pub(super) fn award_kind(
    draw: &games::Draw,
    fell_back: Option<&str>,
) -> &'static str {
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
    // 抽档是纯计算，先定死；再开一笔事务把「扣注额 + 结算」一起提交 ——
    // 分两次写的话，结算失败就是「注额扣了、奖没开」，玩家白亏一注。
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("刮刮乐奖池不可抽样".into()))?;
    let win_idem = format!("game-scratch-win:{}", idem);
    // 幂等：同一键重复提交（网络重试/双击）不重复扣款，也**不重开一次奖** ——
    // 否则「首局未中奖 + 重放中奖」= 白赚，是必须堵住的印钞口。
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    if !matches!(
        spend_spark_tx(&mut tx, auth.id, body.bet, "game", &idem, "scratch", 0)
            .await?,
        SpendOutcome::Spent
    ) {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    let (spark, value, fell_back) = settle_tx(
        &mut tx,
        auth.id,
        body.bet,
        &draw,
        "scratch",
        &win_idem,
        meta_rarity(&pool.meta, draw.index),
    )
    .await?;
    tx.commit().await.map_err(dberr)?;
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

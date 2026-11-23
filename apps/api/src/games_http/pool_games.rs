//! 池抽类玩法：九宫格 / 扭蛋机 / 大转盘（从 casino.rs 拆出：撞 300 行上限）。
//!
//! 三者**同构**（票价 + 加权池抽档），只差 `game`（决定池、流水 ref_type、审计名）。
//! 三处各写一遍必然漂移，故只留 `pool_round` 一份实现，三个 handler 都是薄壳。
//! 结算细节（魔力位入账 / 物品位发放 / 回落折魔力）复用 casino.rs 的 `settle`。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::Row;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::casino::{award_kind, settle_tx};
use super::helpers::{check_minclass, check_rate, eco_i64, idem_key};
use super::pool::{dberr, load_pool};

/// 批量上限：一次事务最多连抽的局数。十连/百连走 pool_batch.rs，
/// 「一直抽」仍逐发单抽（要逐局出闪字演出）。
pub(crate) const MAX_BATCH: i64 = 100;

#[derive(Deserialize)]
// 会员面「花火花」写口：多带的键不静默忽略（P-314 一族，普查见 scripts/pt_audit_write_unknown_field.py）。
// 九宫格/扭蛋/大转盘：票价在池配置里，客户端多带的键被吞后「我以为下的是 X」没有回执。
#[serde(deny_unknown_fields)]
struct DrawReq {
    /// 客户端幂等键（审计 P2-8）
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 已提交局的回放。响应丢失后客户端用同一幂等键重试，必须拿回原结果，
/// 而不是只收到「已受理」再让玩家自己猜钱去哪了。
async fn replay_round(
    db: &sqlx::PgPool,
    idem: &str,
) -> DomainResult<serde_json::Value> {
    // mult 回算：抽中档按「load_pool 同序（p.sort, e.sort）」的第
    // entry_index 行取 mult_permille——回放表没存 mult（历史局也没有），
    // 池档被站长改过时回算的是*当前*池的倍率， prize 文案同此口径。
    let row = sqlx::query(
        "WITH pool AS ( \
           SELECT e.mult_permille, row_number() OVER ( \
             ORDER BY p.sort, e.sort) - 1 AS idx \
           FROM arcade_pools p JOIN arcade_pool_entries e \
             ON e.pool_key = p.key AND e.enabled \
           WHERE p.game = (SELECT game FROM arcade_draw_results \
                           WHERE idempotency_key = $1) AND p.enabled \
         ) \
         SELECT d.entry_index, d.prize, d.rarity, d.kind, d.fell_back, \
                d.replay, d.payout, d.value, d.ticket, pool.mult_permille \
         FROM arcade_draw_results d \
         LEFT JOIN pool ON pool.idx = d.entry_index \
         WHERE d.idempotency_key = $1",
    )
    .bind(idem)
    .fetch_optional(db)
    .await
    .map_err(dberr)?
    .ok_or(DomainError::LedgerConflict)?;
    let index: i32 = row.get("entry_index");
    let prize: String = row.get("prize");
    let rarity: i16 = row.get("rarity");
    let kind: String = row.get("kind");
    let fell_back: Option<String> = row.get("fell_back");
    let replay: bool = row.get("replay");
    let payout: i64 = row.get("payout");
    let value: i64 = row.get("value");
    let ticket: i64 = row.get("ticket");
    let mult_permille: Option<i64> = row.get("mult_permille");
    Ok(serde_json::json!({
        "index": index, "prize": prize, "rarity": rarity, "kind": kind,
        "fell_back": fell_back, "replay": replay,
        "payout": payout, "value": value, "ticket": ticket,
        "net": value - ticket,
        // 与首发的 mult 对齐：回放方也要拿到语言无关倍率读数
        "mult": mult_permille.map(|m| m as f64 / games::MULT_UNIT as f64),
    }))
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
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let r = round_in_tx(&mut tx, uid, game, &pool, &idem).await?;
    if r.is_none() {
        // 幂等命中：这局已受理过，回放原结果（不重抽、不重扣）。
        let _ = tx.rollback().await;
        return replay_round(&state.repo.db, &idem).await;
    }
    let (mult, rarity, idx, label, kind, fell_back, replay, spark, value) =
        r.unwrap();
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(uid), &format!("game.{game}"), None)
        .await;
    Ok(serde_json::json!({
        "index": idx,
        "prize": label,
        "rarity": rarity,
        "kind": kind,
        "fell_back": fell_back,
        "replay": replay,
        "payout": spark,
        "value": value,
        "ticket": ticket,
        "net": value - ticket,
        // 魔力档的语言无关读数（与批量同款）：×N 由客户端组合展示
        "mult": mult,
    }))
}

/// 单局事务段：扣票 → 抽档 → 结算 → 回放，同生共死。
/// 返回 None = 幂等命中（该键已受理过），调用方走 replay_round 回放；
/// 返回 Some(...) = (mult, rarity, index, prize, kind, fell_back,
/// replay, payout, value)。
/// 批量连抽（pool_batch.rs）逐局复用同一段，单局纪律只有这一份。
#[allow(clippy::type_complexity)]
pub(super) async fn round_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    game: &str,
    pool: &super::pool::Pool,
    idem: &str,
) -> DomainResult<
    Option<(
        f64,
        i16,
        usize,
        String,
        &'static str,
        Option<String>,
        bool,
        i64,
        i64,
    )>,
> {
    // 抽档是纯计算，先定死；再开一笔事务把「扣票 + 结算」一起提交 ——
    // 分两次写的话，结算失败就是「票价扣了、奖没开」，玩家白亏一注。
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("奖池不可抽样".into()))?;
    // 魔力位直接入账；物品位走发放账（库存/上限/幂等同一事务判），
    // 发不出去时按 FALLBACK_MULT 折魔力 —— 这条回落已被 EV 计入，不是额外成本。
    let win_idem = format!("game-{game}-win:{idem}");
    // 幂等（同 scratch）：重放不重开，避免「首局没中 + 重放中了」白赚
    if !matches!(
        spend_spark_tx(tx, uid, pool.ticket, "game", idem, game, 0).await?,
        SpendOutcome::Spent
    ) {
        return Ok(None);
    }
    let (spark, value, fell_back) = settle_tx(
        tx,
        uid,
        pool.ticket,
        &draw,
        game,
        &win_idem,
        super::casino::meta_rarity(&pool.meta, draw.index),
    )
    .await?;
    // 同事务落回放结果：扣票、结算、回放三写同生共死；网络丢了响应也追得回。
    let kind = award_kind(&draw, fell_back);
    let rarity = super::casino::meta_rarity(&pool.meta, draw.index);
    // 返本档（倍数恰为 1）不占九宫格灯格：前端灯阵跳过它、直接按返本演出。
    // 只有 jgg 的灯板按格定位中奖，其余两个玩法（扭蛋/大转盘）没有灯阵。
    let replay = game == "jgg"
        && draw.prize.mult_permille() == 1000
        && matches!(draw.prize.kind, games::EntryKind::Magic { .. });
    sqlx::query(
        "INSERT INTO arcade_draw_results (idempotency_key, user_id, game, \
         entry_index, prize, rarity, kind, fell_back, replay, payout, \
         value, ticket) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) \
         ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(idem)
    .bind(uid)
    .bind(game)
    .bind(draw.index as i32)
    .bind(&draw.prize.label)
    .bind(rarity)
    .bind(kind)
    .bind(fell_back)
    .bind(replay)
    .bind(spark)
    .bind(value)
    .bind(pool.ticket)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    let mult = match draw.prize.kind {
        games::EntryKind::Magic { mult_permille } => {
            mult_permille as f64 / games::MULT_UNIT as f64
        }
        _ => 0.0,
    };
    Ok(Some((
        mult,
        rarity,
        draw.index,
        draw.prize.label,
        kind,
        fell_back.map(str::to_owned),
        replay,
        spark,
        value,
    )))
}

#[post("/games/jgg")]
pub(super) async fn jgg(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: crate::http::OptJson<DrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "jgg", auth.class_id).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "jgg", client_idem).await?))
}

/// 老虎机（旧扭蛋池键 capsule 复用：历史流水 / 成就 / 图鉴不断档）。
#[post("/games/capsule")]
pub(super) async fn capsule(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: crate::http::OptJson<DrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "capsule", auth.class_id).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(
        pool_round(&state, auth.id, "capsule", client_idem).await?
    ))
}

/// 水果机：与九宫格同构，仅池键不同（fruit）。
#[post("/games/fruit")]
pub(super) async fn fruit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: crate::http::OptJson<DrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fruit", auth.class_id).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "fruit", client_idem).await?))
}

/// 大转盘：与九宫格同构，仅池键不同（wheel）。
#[post("/games/wheel")]
pub(super) async fn wheel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: crate::http::OptJson<DrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "wheel", auth.class_id).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    Ok(ok(pool_round(&state, auth.id, "wheel", client_idem).await?))
}

#[cfg(test)]
mod tests {
    use super::DrawReq;

    /// `deny_unknown_fields` 的**行为**证据（不是静态断言）：票价写在池
    /// 配置里，客户端多带一个键（以为 `count: 5` 能连抽五局）时，旧写法
    /// 静默忽略、照样扣一张票开一局；现在要求它在解析边界就被拒。
    /// 六个兄弟结构体（CardReq/BetReq/GuessReq/CastReq/DrawReq/DonateReq）
    /// 挂的是同一条 serde 属性，行为一致，取一个作现场。
    #[test]
    fn unknown_field_is_rejected() {
        let good = serde_json::from_str::<DrawReq>(
            r#"{"idempotency_key":"abcdefgh"}"#,
        );
        assert!(good.is_ok(), "合法载荷必须照常受理：{:?}", good.err());
        let bad = serde_json::from_str::<DrawReq>(
            r#"{"idempotency_key":"abcdefgh","count":5}"#,
        );
        let msg = bad.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("unknown field"),
            "应报 unknown field：%{}",
            msg
        );
        assert!(msg.contains("count"), "报错要点名那个键：%{}", msg);
    }
}

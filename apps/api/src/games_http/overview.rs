//! 游戏总览/历史/回合列表（M24 只读面）。
//! 从 games_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_PLAYS_PER_HOUR};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{
    bigsmall_mult_permille, eco_i64, limit_used, scratch_odds,
};

#[get("/games")]
pub(super) async fn games_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let max_bet = eco_i64(&state, "games_max_bet", games::MAX_BET).await;
    let max_plays =
        eco_i64(&state, "games_max_plays_per_hour", MAX_PLAYS_PER_HOUR).await;
    let odds = scratch_odds(&state).await;
    let win_mult = bigsmall_mult_permille(&state).await;
    let jgg_prizes: Vec<_> = games::JGG_PRIZES
        .iter()
        .map(|p| {
            serde_json::json!({
                "label": p.label,
                "weight_permille": p.weight,
                "payout": p.payout,
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "max_bet": max_bet,
        "max_plays_per_hour": max_plays,
        "scratch": { "name": "刮刮乐", "max_bet": max_bet, "prizes": [
            { "multiplier": 0.5, "pct": odds.half },
            { "multiplier": 1.0, "pct": odds.one },
            { "multiplier": 2.0, "pct": odds.two },
            { "multiplier": 10.0, "pct": odds.ten }
        ], "empty_pct": odds.empty },
        "bigsmall": { "name": "猜大小", "max_bet": max_bet,
            "win_mult": win_mult as f64 / 1000.0,
            "expected_value": games::bigsmall_expected_value(win_mult),
            "rule": "1-49 小 · 52-100 大 · 50/51 平局返本 · 猜中按赔率派彩" },
        "jgg": { "name": "九宫格抽奖", "ticket": games::JGG_TICKET, "prizes": jgg_prizes },
        "farm": { "name": "农场", "slots": 6, "market_refresh": "每日 0/4/8/12/16/20 点", "volatility": "±50%" },
        "funvote": { "name": "趣味盒投票", "cost": "1 魔力/票", "rule": "一人一票" },
        "rate_limit": format!("每人每小时 {max_plays} 次"),
    });

    // 登录态补齐：余额 / 今日战绩 / 剩余局数（前端「下注前先看得见」的依赖）
    if let Ok(auth) = require_auth(&req, &state).await {
        let balance: i64 =
            sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .unwrap_or(0);
        // 今日口径 UTC+8（与签到/统计一致）。sum(bigint) 在 PG 里是 NUMERIC，必须显式转 bigint
        let today: (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(sum(amount), 0)::bigint, \
                    (count(*) FILTER (WHERE amount < 0))::bigint \
             FROM spark_ledger WHERE user_id = $1 AND kind = 'game' \
             AND created_at >= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai')",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        body["me"] = serde_json::json!({
            "balance": balance,
            "today_net": today.0,
            "today_plays": today.1,
            // 剩余局数取**限流器本身**的计数（INCR + EXPIRE 3600 的滚动小时窗口），
            // 不是「今日局数」——两者口径不同，用今日局数推算会与实际限流不符。
            "limit_left": (max_plays - limit_used(&state, auth.id, false).await.unwrap_or(today.1)).max(0),
        });
    }
    Ok(ok(body))
}

/// 我的游戏战绩（最近 N 条流水，来自 spark_ledger —— 不新建表、不本地累积）。
/// kind='game' 的下注为负、派彩为正；`game` 参数对应 ref_type（scratch/bigsmall/jgg/farm_plant）。
#[derive(sqlx::FromRow, serde::Serialize)]
struct GameHistoryRow {
    ref_type: Option<String>,
    amount: i64,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    game: Option<String>,
    limit: Option<i64>,
}

#[get("/games/history")]
pub(super) async fn game_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<HistoryQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<GameHistoryRow> = sqlx::query_as(
        "SELECT ref_type, amount, balance_after, created_at FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'game' \
           AND ($2::text IS NULL OR ref_type = $2) \
         ORDER BY id DESC LIMIT $3",
    )
    .bind(auth.id)
    .bind(&q.game)
    .bind(q.limit.unwrap_or(20).clamp(1, 50))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 我的**对局**战绩（一局一条，不是流水条）。
///
/// 为什么需要它：即时玩法每局写两条流水（下注为负、派彩为正），直接拿流水画路单会
/// 一半是负一半是正、局数还翻倍，看起来像「输多赢少」。这里以「下注流水」为主体，
/// 按幂等键前缀 LEFT JOIN 出该局的派彩，返回 `bet / payout / net`。
/// 幂等键前缀历史遗留两种写法（`game-bs-win:` 与 `game-<ref_type>-win:`），一并兼容。
#[derive(sqlx::FromRow, serde::Serialize)]
struct GameRoundRow {
    game: String,
    bet: i64,
    payout: i64,
    net: i64,
    at: chrono::DateTime<chrono::Utc>,
}

#[get("/games/rounds")]
pub(super) async fn game_rounds(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<HistoryQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<GameRoundRow> = sqlx::query_as(
        "SELECT l.ref_type AS game, \
                l.amount AS bet, \
                COALESCE(w.amount, 0) AS payout, \
                l.amount + COALESCE(w.amount, 0) AS net, \
                l.created_at AS at \
         FROM spark_ledger l \
         LEFT JOIN spark_ledger w \
           ON w.idempotency_key IN ( \
                'game-' || l.ref_type || '-win:' || l.idempotency_key, \
                'game-bs-win:' || l.idempotency_key \
              ) \
         WHERE l.user_id = $1 AND l.kind = 'game' AND l.amount < 0 \
           AND l.ref_type IN ('scratch', 'bigsmall', 'jgg') \
           AND ($2::text IS NULL OR l.ref_type = $2) \
           AND l.created_at > now() - interval '90 days' \
         ORDER BY l.id DESC LIMIT $3",
    )
    .bind(auth.id)
    .bind(&q.game)
    .bind(q.limit.unwrap_or(20).clamp(1, 50))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

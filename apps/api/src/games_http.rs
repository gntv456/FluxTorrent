//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小/九宫格 + 农场 + 每小时限次）。
//! 经济定位：四玩法一律**回收魔力**（各自 EV < 1），赔率/概率走代码常量 + 设置键双源。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy_http::{
    earn_spark, earn_spark_tx, spend_spark, SpendOutcome,
};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, Guess, MAX_PLAYS_PER_HOUR};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_games(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(games_overview)
        .service(game_history)
        .service(game_rounds)
        .service(scratch)
        .service(guess_bigsmall)
        .service(jgg)
        .service(farm_overview)
        .service(farm_plant)
        .service(farm_water)
        .service(farm_harvest)
        .service(fun_polls)
        .service(fun_vote)
}

/// 娱乐屋总览（匿名可读规则，登录额外回「我的」）。
/// 奖池表一律由代码常量生成 —— 修「前端硬编码 100x 与后端 JGG_PRIZES 不符」：
/// 展示与实现同源，站长调 EV 单测后前端自动跟着变。
#[get("/games")]
async fn games_overview(
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
async fn game_history(
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
async fn game_rounds(
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

/// 读取游戏经济设置键（0109 参数化；缺省回落代码默认值 T3）
async fn eco_i64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: i64,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = $1)::bigint, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 浮点设置键（赔率类支持小数，如猜大小 1.9x）
async fn eco_f64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: f64,
) -> f64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = $1)::double precision, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 刮刮乐档位（五档可配；10x 留空/合计不为 100 时按余数推导，缺省 45/30/15/8/2）
async fn scratch_odds(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> games::ScratchOdds {
    games::ScratchOdds::from_parts(
        eco_i64(state, "games_scratch_empty_pct", 45).await,
        eco_i64(state, "games_scratch_half_pct", 30).await,
        eco_i64(state, "games_scratch_one_pct", 15).await,
        eco_i64(state, "games_scratch_two_pct", 8).await,
        eco_i64(state, "games_scratch_ten_pct", 2).await,
    )
}

/// 农场作物有效期（天；0 = 永不枯萎）
async fn farm_wither_days(state: &web::Data<std::sync::Arc<AppState>>) -> i64 {
    eco_i64(state, "farm_wither_days", 5).await.clamp(0, 60)
}

/// 猜大小赔率（千分比）。倍数设置键缺省 1.9 —— **必须 < 2.0**：
/// 2.0 时 EV 恰为 1.0（不回收）且可双向零风险对冲，见 games.rs 常量说明。
/// 上限钳到 1999‰（运行时 EV 防线，P2）：设置键是管理员可写参数，此前上限 10_000‰
/// 意味着误配/越权写入 ≥2000‰ 即把游戏变成增发开关。
async fn bigsmall_mult_permille(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    let mult = eco_f64(state, "games_bigsmall_win_mult", 1.9).await;
    ((mult * 1000.0).round() as i64).clamp(0, 1_999)
}

/// 限流作用域：即时赌局与农场**分开计数**。
/// 农场是慢玩法，一次种满 6 块地不该吃掉 6 次即时下注额度（旧实现共用 `rl:games`）。
#[derive(Clone, Copy)]
enum RateScope {
    Instant,
    Farm,
}

impl RateScope {
    fn redis_key(&self, user_id: i64) -> String {
        match self {
            RateScope::Instant => format!("rl:games:{user_id}"),
            RateScope::Farm => format!("rl:farm:{user_id}"),
        }
    }
    fn setting(&self) -> (&'static str, i64) {
        match self {
            RateScope::Instant => {
                ("games_max_plays_per_hour", MAX_PLAYS_PER_HOUR)
            }
            RateScope::Farm => ("farm_max_plays_per_hour", 30),
        }
    }
}

/// 每小时限次（Redis INCR + EXPIRE；上限走设置键）。
/// 审计修复（P2-7）：Redis 不可用时**拒绝**（fail-close）——旧实现 `unwrap_or(0)` 会让限流静默失效。
async fn check_rate_scoped(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
    scope: RateScope,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let (setting, default) = scope.setting();
    let limit = eco_i64(state, setting, default).await;
    let key = scope.redis_key(user_id);
    let mut conn = redis.clone();
    let n: i64 = conn.incr(&key, 1).await.map_err(|_| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
    })?;
    if n == 1 {
        let _: () = conn.expire(&key, 3600).await.map_err(|_| {
            DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
        })?;
    }
    if n > limit {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

async fn check_rate(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
) -> DomainResult<()> {
    check_rate_scoped(state, redis, user_id, RateScope::Instant).await
}

/// 读取限流器当前已用次数（只读，不 +1；供前端显示「剩余 N 局」）。
/// 取不到（Redis 未计数/异常）返回 None，调用方自行回落。
async fn limit_used(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    farm: bool,
) -> Option<i64> {
    use redis::AsyncCommands;
    let key = if farm {
        RateScope::Farm.redis_key(user_id)
    } else {
        RateScope::Instant.redis_key(user_id)
    };
    let mut conn = state.redis.clone();
    conn.get::<_, Option<i64>>(&key).await.ok().flatten()
}

/// 下注校验（上限走设置键 games_max_bet）
async fn check_bet(
    state: &web::Data<std::sync::Arc<AppState>>,
    bet: i64,
) -> Result<(), String> {
    let max = eco_i64(state, "games_max_bet", games::MAX_BET).await;
    if bet <= 0 {
        return Err("下注必须为正数".into());
    }
    if bet > max {
        return Err(format!("单次下注不能超过 {max} 魔力"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct BetReq {
    bet: i64,
    /// 客户端幂等键（审计 P2-8）：网络重试/双击须带同一键，缺省回落随机键保持兼容
    #[serde(default)]
    idempotency_key: Option<String>,
}

fn idem_key(prefix: &str, user_id: i64, client: &Option<String>) -> String {
    match client {
        Some(k) if !k.trim().is_empty() && k.len() <= 128 => {
            format!("game-{prefix}:{user_id}:{k}")
        }
        _ => format!("game-{prefix}:{}:{}", user_id, Uuid::new_v4()),
    }
}

#[post("/games/scratch")]
async fn scratch(
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
    let odds = scratch_odds(&state).await;
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
async fn guess_bigsmall(
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
async fn jgg(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<JggReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 风控一致性：票价同样受「单次下注上限」约束（旧实现绕过 games_max_bet，
    // 站长把上限调到 100 以下时仍能抽走 100）。先校验再计数，避免白耗次数。
    let ticket = games::JGG_TICKET;
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
    let draw = games::jgg_draw();
    let payout = ticket * draw.prize.payout;
    if payout > 0 {
        let win_idem = format!("game-jgg-win:{}", idem);
        earn_spark(&state.repo.db, auth.id, payout, "game", &win_idem).await?;
    }
    state.repo.audit(Some(auth.id), "game.jgg", None).await;
    Ok(ok(serde_json::json!({
        "index": draw.index,
        "prize": draw.prize.label,
        "payout": payout,
        "ticket": ticket,
        "net": payout - ticket,
    })))
}

// ============ 农场 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct CropRow {
    id: i32,
    name: String,
    seed_price: i32,
    base_yield: i32,
    grow_hours: i32,
    market_price: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PlotRow {
    slot: i32,
    crop_id: i32,
    crop_name: String,
    planted_at: chrono::DateTime<chrono::Utc>,
    ready_at: chrono::DateTime<chrono::Utc>,
    watered: bool,
    ready: bool,
    /// 成熟后超过 `farm_wither_days` 天未收获 → 枯萎（收获作废、清空地块）
    withered: bool,
}

async fn get_crop(
    db: &sqlx::PgPool,
    crop_id: i32,
) -> DomainResult<Option<CropRow>> {
    sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, 0::bigint AS market_price FROM farm_crops WHERE id = $1",
    )
    .bind(crop_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 农场总览：作物行情（含当前窗口市场价）+ 我的 6 块地
#[get("/farm")]
async fn farm_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);

    let crops: Vec<CropRow> = sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, 0::bigint AS market_price FROM farm_crops ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let crops: Vec<_> = crops
        .into_iter()
        .map(|mut c| {
            c.market_price = games::market_price(c.seed_price as i64, window);
            c
        })
        .collect();

    let wither_days = farm_wither_days(&state).await;
    let plots: Vec<PlotRow> = sqlx::query_as(
        r#"SELECT p.slot, p.crop_id, c.name AS crop_name, p.planted_at, p.ready_at, p.watered,
            (p.ready_at <= now()) AS ready,
            ($2 > 0 AND p.ready_at + make_interval(days => $2::int) < now()) AS withered
         FROM farm_plots p JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 ORDER BY p.slot"#,
    )
    .bind(auth.id)
    .bind(wither_days)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 农场自己的限流配额（rl:farm，与即时赌局分开计数）——前端显示「今日可操作 N 次」
    let farm_limit = eco_i64(&state, "farm_max_plays_per_hour", 30).await;
    let farm_left = (farm_limit
        - limit_used(&state, auth.id, true).await.unwrap_or(0))
    .max(0);

    Ok(ok(serde_json::json!({
        "window_start": window,
        "next_refresh": window + 4 * 3600,
        "crops": crops,
        "plots": plots,
        "slots": 6,
        "wither_days": wither_days,
        "hour_limit": farm_limit,
        "hour_left": farm_left,
    })))
}

#[derive(Deserialize)]
struct PlantReq {
    slot: i32,
    crop_id: i32,
}

#[post("/farm/plant")]
async fn farm_plant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PlantReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 审计修复（P1 印钞）：农场种植此前不进 games 限流（对照 scratch/dice/jgg），
    // 确定性市场价可被脚本以 6 槽 × 高频轮种套取波动收益。现独立计数（rl:farm），
    // 与即时赌局额度分开 —— 否则种满 6 块地就吃掉 6 次下注额度。
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Farm).await?;
    if !(1..=6).contains(&body.slot) {
        return Err(DomainError::Validation("slot 取值 1-6".into()));
    }
    let Some(crop) = get_crop(&state.repo.db, body.crop_id).await? else {
        return Err(DomainError::Validation("作物不存在".into()));
    };

    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);
    let price = games::market_price(crop.seed_price as i64, window);

    // 买种经统一交易管线扣款（幂等键含用户+槽位+当前分钟）。
    // 审计修复（P0 铸币）：旧逻辑对 spend_spark 返回的 Replayed 不检查——同槽同分钟内
    // 第二个请求（换高价作物）不扣款即走到占位失败分支，再按"本次请求的高价"全额退款。
    // 现在：Replayed 直接拒绝（本请求未付费），退款金额以幂等键对应的实际扣款额为准。
    let idem = format!("farm-plant:{}:{}:{}", auth.id, body.slot, now / 60);
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "game",
        &idem,
        "farm_plant",
        crop.id as i64,
    )
    .await?;
    if !matches!(outcome, SpendOutcome::Spent) {
        return Err(DomainError::Validation(
            "操作过于频繁，请一分钟后再试".into(),
        ));
    }

    let planted = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO farm_plots (user_id, slot, crop_id, ready_at)
         VALUES ($1, $2, $3, now() + make_interval(hours => $4))
         ON CONFLICT (user_id, slot) DO NOTHING RETURNING id"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(crop.id)
    .bind(crop.grow_hours)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if planted.is_none() {
        // 占位失败（槽位已被占）：退款补偿。退款额必须取该幂等键的实际扣款额，
        // 而非本次请求价——两者在本请求 Replayed 已被拒绝的前提下仍可能有差异
        //（同分钟内首请求是低价作物），按实际扣款退才能保证净 0。
        let actual: Option<i64> = sqlx::query_scalar(
            "SELECT amount FROM spark_ledger WHERE idempotency_key = $1 AND user_id = $2 LIMIT 1",
        )
        .bind(&idem)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let refund_amt = actual.unwrap_or(price);
        let refund = format!("farm-refund:{}", idem);
        earn_spark(&state.repo.db, auth.id, refund_amt, "game", &refund)
            .await?;
        return Err(DomainError::Validation("该地块已有作物".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "farm.plant", Some(crop.id as i64))
        .await;
    Ok(ok(serde_json::json!({
        "slot": body.slot, "crop": crop.name, "cost": price,
        "ready_at": (chrono::Utc::now() + chrono::Duration::hours(crop.grow_hours as i64)).to_rfc3339(),
    })))
}

#[derive(Deserialize)]
struct SlotReq {
    slot: i32,
}

#[post("/farm/water")]
async fn farm_water(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 枯萎地块不能浇水（有效期已过，救不回来）
    let wither_days = farm_wither_days(&state).await;
    let updated = sqlx::query(
        r#"UPDATE farm_plots SET watered = TRUE, ready_at = ready_at - interval '10 minutes'
         WHERE user_id = $1 AND slot = $2 AND watered = FALSE
           AND ($3 = 0 OR ready_at + make_interval(days => $3::int) >= now())"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(wither_days)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "该地块无需浇水（未种植 / 已浇过 / 已枯萎）".into(),
        ));
    }
    // 浇水消耗（0109 键 farm_water_spark，缺省 1；为 0 表示免费）。
    // 先占位后扣费，扣费失败回滚占位（与 fun_vote 同口径，避免白扣或白浇）。
    let cost = eco_i64(&state, "farm_water_spark", 1).await;
    if cost > 0 {
        let idem = format!("farm-water:{}:{}", auth.id, body.slot);
        if let Err(e) = spend_spark(
            &state.repo.db,
            auth.id,
            cost,
            "game",
            &idem,
            "farm_water",
            body.slot as i64,
        )
        .await
        {
            let _ = sqlx::query(
                r#"UPDATE farm_plots SET watered = FALSE, ready_at = ready_at + interval '10 minutes'
                   WHERE user_id = $1 AND slot = $2"#,
            )
            .bind(auth.id)
            .bind(body.slot)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    }
    Ok(ok(
        serde_json::json!({ "watered": true, "accelerated_minutes": 10, "cost": cost }),
    ))
}

#[post("/farm/harvest")]
async fn farm_harvest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let wither_days = farm_wither_days(&state).await;
    // 单事务（P1 收口）：地块行锁、入账、清地块、留痕同生共死。旧版 FOR UPDATE 用在
    // autocommit 连接上锁立即失效（重复收获实际只靠 earn 幂等键兜底），且 earn 与
    // DELETE/留痕分属多个事务，中途失败会留下「钱发了地还在/地没了账没记」的中间态。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 行级锁防重复收获（先 FOR UPDATE 拿到成熟地块）；`FOR UPDATE OF p` 只锁地块表，
    // 不锁 crops（否则所有收同一作物的用户会互相串行）
    let plot: Option<(i64, i32, String, bool)> = sqlx::query_as(
        r#"SELECT p.id, p.crop_id, c.name AS crop_name,
             ($3 > 0 AND p.ready_at + make_interval(days => $3::int) < now()) AS withered
         FROM farm_plots p JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 AND p.slot = $2 AND p.ready_at <= now() FOR UPDATE OF p"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .bind(wither_days)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plot_id, crop_id, crop_name, withered)) = plot else {
        return Err(DomainError::Validation("该地块尚未成熟".into()));
    };

    // 枯萎（超过有效期未收）：收获作废（0 魔力），但**清空地块**让玩家能重种。
    // 记账留痕（amount=0 的收获流水），便于运营统计浪费。
    if withered {
        sqlx::query("DELETE FROM farm_plots WHERE id = $1")
            .bind(plot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO farm_harvests (user_id, crop_id, amount, market_price, doubled) VALUES ($1, $2, 0, 0, FALSE)",
        )
        .bind(auth.id)
        .bind(crop_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        state
            .repo
            .audit(Some(auth.id), "farm.wither", Some(crop_id as i64))
            .await;
        return Ok(ok(serde_json::json!({
            "crop": crop_name, "amount": 0, "market_price": 0, "doubled": false, "withered": true,
        })));
    }

    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);
    let crop = get_crop(&state.repo.db, crop_id)
        .await?
        .ok_or(DomainError::Validation("作物不存在".into()))?;
    // 收获量 = base_yield × 收获侧市场因子（±50% 窗口波动；与买种侧因子错开——
    // 见 games::harvest_market_price 注释，消除确定性低买高卖套利）。
    // 回收口径：作物表按「产量 = 种子价 × 0.75」标定，含 20% 双倍后期望回报 0.90 < 1
    // （0127 迁移统一下发，五档一致；市场 ±50% 只影响单局运气，不改期望）。
    let market = games::harvest_market_price(crop.base_yield as i64, window);

    let doubled = games::roll_double();
    let amount = if doubled { market * 2 } else { market };

    // 收益经统一交易管线入账（幂等键绑定地块；与地块锁同事务，双重防重复收获。
    // 地块已在本事务锁定且即将删除，重放不可达；显式丢弃以满足 must_use 契约）
    let idem = format!("farm-harvest:{}", plot_id);
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, amount, "game", &idem).await?;
    let _ = earn_outcome;

    sqlx::query("DELETE FROM farm_plots WHERE id = $1")
        .bind(plot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO farm_harvests (user_id, crop_id, amount, market_price, doubled) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(auth.id)
    .bind(crop_id)
    .bind(amount)
    .bind(market)
    .bind(doubled)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "crop": crop.name, "amount": amount, "market_price": market, "doubled": doubled, "withered": false,
    })))
}

// ============ 趣味盒投票（funvote 口径：投票 +1 火花） ============

#[derive(sqlx::FromRow)]
struct FunPollRow {
    id: i64,
    question: String,
    options: serde_json::Value,
    closed: bool,
    my_vote: Option<i32>,
    total: i64,
}

#[get("/fun/polls")]
async fn fun_polls(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<FunPollRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.options, p.closed,             (SELECT v.option_index FROM fun_votes v WHERE v.poll_id = p.id AND v.user_id = $1) AS my_vote,             (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS total          FROM fun_polls p WHERE NOT p.closed ORDER BY p.id LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 附每项计数
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let counts: Vec<(i32, i64)> = sqlx::query_as(
            "SELECT option_index, count(*) FROM fun_votes WHERE poll_id = $1 GROUP BY option_index",
        )
        .bind(r.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        out.push(serde_json::json!({
            "id": r.id, "question": r.question, "options": r.options,
            "closed": r.closed, "my_vote": r.my_vote, "total_votes": r.total,
            "counts": counts.into_iter().map(|(i, c)| serde_json::json!({"index": i, "votes": c})).collect::<Vec<_>>(),
        }));
    }
    Ok(ok(out))
}

#[derive(Deserialize)]
struct FunVoteReq {
    poll_id: i64,
    option_index: i32,
}

#[post("/fun/vote")]
async fn fun_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 审计修复（P2 竞态）：旧版先 INSERT 占位、校验失败再 DELETE 回滚——并发下一人的
    // 合法占位可能被另一人的非法回滚误删。校验全部前置，通过后才落占位。
    let valid: Option<(serde_json::Value, bool)> =
        sqlx::query_as("SELECT options, closed FROM fun_polls WHERE id = $1")
            .bind(body.poll_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((options, closed)) = valid else {
        return Err(DomainError::Validation("投票不存在".into()));
    };
    let n = options.as_array().map(|a| a.len()).unwrap_or(0);
    if closed {
        return Err(DomainError::Validation("投票已结束".into()));
    }
    if body.option_index < 0 || body.option_index as usize >= n {
        return Err(DomainError::Validation("选项无效".into()));
    }
    let voted = sqlx::query(
        "INSERT INTO fun_votes (poll_id, user_id, option_index) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(body.poll_id)
    .bind(auth.id)
    .bind(body.option_index)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if voted.rows_affected() == 0 {
        return Err(DomainError::Validation("已经投过啦，一人一票".into()));
    }
    // 投票 +1 火花（旧站口径），扣款失败回滚占位
    let idem = format!("fun-vote:{}:{}", auth.id, body.poll_id);
    if let Err(e) = spend_spark(
        &state.repo.db,
        auth.id,
        1,
        "vote",
        &idem,
        "fun_poll",
        body.poll_id,
    )
    .await
    {
        let _ = sqlx::query(
            "DELETE FROM fun_votes WHERE poll_id = $1 AND user_id = $2",
        )
        .bind(body.poll_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
        return Err(e);
    }
    Ok(ok(
        serde_json::json!({ "voted": body.option_index, "cost": 1 }),
    ))
}

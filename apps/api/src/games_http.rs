//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小/九宫格 + 好学农场 + 每小时限次）。

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
        .service(jgg)
        .service(farm_overview)
        .service(farm_plant)
        .service(farm_water)
        .service(farm_harvest)
        .service(fun_polls)
        .service(fun_vote)
}

#[get("/games")]
async fn games_overview() -> impl Responder {
    ok(serde_json::json!({
        "scratch": { "name": "刮刮乐", "max_bet": games::MAX_BET,
            "prizes": ["0.5x (30%)", "1x (15%)", "2x (8%)", "10x (2%)"] },
        "bigsmall": { "name": "猜大小", "max_bet": games::MAX_BET,
            "rule": "1-49 小 · 52-100 大 · 50/51 平局返本 · 猜中 2x" },
        "jgg": { "name": "九宫格抽奖", "ticket": games::JGG_TICKET,
            "prizes": ["谢谢参与 38%", "再来一次 12%", "2x 20%", "3x 12%", "5x 10%", "10x 5.5%", "50x 2%", "100x 0.5%"] },
        "farm": { "name": "好学农场", "slots": 6, "market_refresh": "每日 0/4/8/12/16/20 点", "volatility": "±50%" },
        "funvote": { "name": "趣味盒投票", "cost": "1 火花/票", "rule": "一人一票" },
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

// ============ 九宫格抽奖（jgg 口径） ============

#[post("/games/jgg")]
async fn jgg(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_rate(&state.redis, auth.id).await?;

    let ticket = games::JGG_TICKET;
    let idem = format!("game-jgg:{}:{}", auth.id, Uuid::new_v4());
    spend_spark(&state.repo.db, auth.id, ticket, "game", &idem, "jgg", 0).await?;
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

// ============ 好学农场（magic_fram 口径） ============

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
}

async fn get_crop(db: &sqlx::PgPool, crop_id: i32) -> DomainResult<Option<CropRow>> {
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

    let plots: Vec<PlotRow> = sqlx::query_as(
        r#"SELECT p.slot, p.crop_id, c.name AS crop_name, p.planted_at, p.ready_at, p.watered,
            (p.ready_at <= now()) AS ready
         FROM farm_plots p JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 ORDER BY p.slot"#,
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "window_start": window,
        "next_refresh": window + 4 * 3600,
        "crops": crops,
        "plots": plots,
        "slots": 6,
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
    if !(1..=6).contains(&body.slot) {
        return Err(DomainError::Validation("slot 取值 1-6".into()));
    }
    let Some(crop) = get_crop(&state.repo.db, body.crop_id).await? else {
        return Err(DomainError::Validation("作物不存在".into()));
    };

    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);
    let price = games::market_price(crop.seed_price as i64, window);

    // 买种经统一交易管线扣款（幂等键含用户+槽位+当前分钟）
    let idem = format!("farm-plant:{}:{}:{}", auth.id, body.slot, now / 60);
    spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "game",
        &idem,
        "farm_plant",
        crop.id as i64,
    )
    .await?;

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
        // 占位失败（槽位已被占）：退款补偿（幂等）
        let refund = format!("farm-refund:{}", idem);
        earn_spark(&state.repo.db, auth.id, price, "game", &refund).await?;
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
    let updated = sqlx::query(
        r#"UPDATE farm_plots SET watered = TRUE, ready_at = ready_at - interval '10 minutes'
         WHERE user_id = $1 AND slot = $2 AND watered = FALSE"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "该地块无需浇水（未种植或已浇过）".into(),
        ));
    }
    Ok(ok(
        serde_json::json!({ "watered": true, "accelerated_minutes": 10 }),
    ))
}

#[post("/farm/harvest")]
async fn farm_harvest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SlotReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 行级锁防重复收获（先 FOR UPDATE 拿到成熟地块）
    let plot: Option<(i64, i32, i32)> = sqlx::query_as(
        r#"SELECT p.id, p.crop_id, c.base_yield FROM farm_plots p
         JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 AND p.slot = $2 AND p.ready_at <= now() FOR UPDATE"#,
    )
    .bind(auth.id)
    .bind(body.slot)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plot_id, crop_id, _base)) = plot else {
        return Err(DomainError::Validation("该地块尚未成熟".into()));
    };

    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);
    let crop = get_crop(&state.repo.db, crop_id)
        .await?
        .ok_or(DomainError::Validation("作物不存在".into()))?;
    let market = games::market_price(crop.seed_price as i64, window);

    let doubled = games::roll_double();
    let amount = if doubled { market * 2 } else { market };

    // 收益经统一交易管线入账（幂等键绑定地块，天然防重复收获）
    let idem = format!("farm-harvest:{}", plot_id);
    earn_spark(&state.repo.db, auth.id, amount, "game", &idem).await?;

    sqlx::query("DELETE FROM farm_plots WHERE id = $1")
        .bind(plot_id)
        .execute(&state.repo.db)
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
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "crop": crop.name, "amount": amount, "market_price": market, "doubled": doubled,
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
    // 占位（一人一票）
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
    // 选项合法性（json 数组下标）+ 未关闭
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
    if closed || body.option_index < 0 || body.option_index as usize >= n {
        // 回滚占位
        let _ = sqlx::query("DELETE FROM fun_votes WHERE poll_id = $1 AND user_id = $2")
            .bind(body.poll_id)
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
        return Err(DomainError::Validation(if closed {
            "投票已结束".into()
        } else {
            "选项无效".into()
        }));
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
        let _ = sqlx::query("DELETE FROM fun_votes WHERE poll_id = $1 AND user_id = $2")
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

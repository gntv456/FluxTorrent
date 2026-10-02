//! 游戏总览/历史/回合列表（M24 只读面）。
//! 从 games_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_PLAYS_PER_HOUR};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{eco_i64, limit_used, market_refresh_text};
use super::prize_view::{prize_rows, region_rows, scratch_empty_pct};

#[get("/games")]
pub(super) async fn games_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let max_bet = eco_i64(&state, "games_max_bet", games::MAX_BET).await;
    let max_plays =
        eco_i64(&state, "games_max_plays_per_hour", MAX_PLAYS_PER_HOUR).await;
    // 刮刮乐与九宫格读同一张奖池行表（0248）：公示、闸门、玩法三处同一份数据
    let spool = super::pool::load_pool(&state.repo.db, "scratch").await?;
    let btable = super::pool::load_table(&state.repo.db, "bigsmall").await?;
    // 赔率读数：赢区魔力位的加权平均（配了物品位时它只是「魔力那一侧」的均值）
    let win_mult = {
        let w: i64 = btable.win.iter().map(|e| i64::from(e.weight)).sum();
        let s: i64 = btable
            .win
            .iter()
            .map(|e| i64::from(e.weight) * e.mult_permille())
            .sum();
        if w == 0 {
            0
        } else {
            s / w
        }
    };
    let pool = super::pool::load_pool(&state.repo.db, "jgg").await?;
    // 物品图标与用途：目录 arcade_items 是唯一权威，这里只是把 icon 与用途附到
    // 灯阵与公示要用的行上（价值口径仍走 load_pool 的 anchor JOIN，不重复取）。
    // 绑 SKU 的行把 SKU 名一起带出：公示要说清「抽到之后你实际拿到什么」。
    let icons: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT i.key, i.icon, i.use_kind, s.name
          FROM arcade_items i
          LEFT JOIN shop_items s
                 ON i.use_kind = 'sku' AND s.id::text = i.use_ref
         WHERE i.enabled
        "#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(super::pool::dberr)?;
    // 公示必须把物品位连同其折算价值一起列出来：只报魔力倍数会让玩家以为物品档不值钱，
    // 也让站长的 EV 复核对不上账。两个玩法共用同一份投影（见 prize_rows）。
    let jgg_prizes = prize_rows(&pool.entries, pool.ticket, &icons, &pool.meta);
    let scratch_prizes =
        prize_rows(&spool.entries, spool.ticket, &icons, &spool.meta);
    // 农场收获彩蛋：确定性收获（0.90）之上的那一档，与三个抽奖读同一张池表
    let fpool = super::farm_egg::load_farm(&state.repo.db).await?;
    let farm_prizes = prize_rows(&fpool.entries, fpool.unit, &icons, &[]);
    let farm_ev = games::farm_total_ev(&fpool.entries, fpool.unit);
    // 扭蛋机 / 大转盘：与九宫格同构，读同一张池表。缺池时给空池而非让整个总览
    // 500 —— 这两池是新玩法，旧库未跑迁移时不该拖垮已有四玩法的大厅。
    let cpool = super::pool::load_pool(&state.repo.db, "capsule")
        .await
        .unwrap_or(super::pool::Pool {
            ticket: 0,
            entries: Vec::new(),
            meta: Vec::new(),
        });
    let wpool = super::pool::load_pool(&state.repo.db, "wheel")
        .await
        .unwrap_or(super::pool::Pool {
            ticket: 0,
            entries: Vec::new(),
            meta: Vec::new(),
        });
    let capsule_prizes =
        prize_rows(&cpool.entries, cpool.ticket, &icons, &cpool.meta);
    let wheel_prizes =
        prize_rows(&wpool.entries, wpool.ticket, &icons, &wpool.meta);
    let fishpool = super::pool::load_pool(&state.repo.db, "fishing")
        .await
        .unwrap_or(super::pool::Pool {
            ticket: 0,
            entries: Vec::new(),
            meta: Vec::new(),
        });
    let fishing_prizes = prize_rows(
        &fishpool.entries,
        fishpool.ticket,
        &icons,
        &fishpool.meta,
    );
    // 大厅玩法清单（顺序/显隐）：表空则前端兜底注册表接管
    let registry = super::arcade_games::load_registry(&state.repo.db)
        .await
        .unwrap_or_default();

    let mut body = serde_json::json!({
        "max_bet": max_bet,
        "registry": registry,
        "max_plays_per_hour": max_plays,
        "scratch": { "name": "刮刮乐", "max_bet": max_bet,
            "min_bet": spool.ticket,
            "ticket": spool.ticket,
            "prizes": scratch_prizes,
            "empty_pct": scratch_empty_pct(&spool.entries),
            "expected_value": games::pool_ev(&spool.entries, spool.ticket) },
        "bigsmall": { "name": "猜大小", "max_bet": max_bet,
            "ticket": btable.ticket,
            "min_bet": btable.ticket,
            "win_mult": win_mult as f64 / 1000.0,
            "prizes": region_rows(&btable, &icons),
            "expected_value": games::pool_ev(&btable.all(), btable.ticket),
            "rule": "3-10 小 · 11-18 大 · 三同豹子判负 · 猜中按赔率派彩" },
        "jgg": { "name": "九宫格抽奖", "ticket": pool.ticket,
            "prizes": jgg_prizes,
            "expected_value": games::pool_ev(&pool.entries, pool.ticket) },
        "farm": { "name": "农场", "slots": games::FARM_PLOTS,
            "market_refresh": market_refresh_text(
                super::helpers::farm_market_hours(&state).await),
            "volatility": "±50%",
            // 彩蛋档的倍数以「这一株作物的种子价」为定标单位，不是票价
            "unit": fpool.unit,
            "prizes": farm_prizes,
            "expected_value": farm_ev },
        "capsule": { "name": "扭蛋机", "ticket": cpool.ticket,
            "prizes": capsule_prizes,
            "expected_value": games::pool_ev(&cpool.entries, cpool.ticket) },
        "wheel": { "name": "大转盘", "ticket": wpool.ticket,
            "prizes": wheel_prizes,
            "expected_value": games::pool_ev(&wpool.entries, wpool.ticket) },
        "fishing": { "name": "钓鱼", "ticket": fishpool.ticket,
            "prizes": fishing_prizes,
            "expected_value": games::pool_ev(
                &fishpool.entries, fishpool.ticket) },
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
        // 猜大小道具栏：把「玩家当前挂得上的道具」随总览一起下发，
        // 前端不必再单拉一个端点（口径与 held 同一份：发放账 − 消耗账）
        let props = super::bigsmall_props::held(&state.repo.db, auth.id)
            .await
            .unwrap_or_default();
        body["props"] = serde_json::json!(props
            .iter()
            .map(|(p, n)| serde_json::json!({
                "key": p.key, "name": p.name, "icon": p.icon,
                "effect": p.effect, "value": p.value, "held": n,
            }))
            .collect::<Vec<_>>());
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
           AND l.ref_type IN ('scratch', 'bigsmall', 'jgg', 'capsule', 'wheel', 'fishing') \
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

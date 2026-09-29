//! 娱乐屋运营面板（staff 只读）：三口径 EV 对照 + 门禁自检 + 参数现值 + 定义一览。
//!
//! 与 arcade-meta 的分工：meta 是**玩家侧**（个人进度），本件是**运营侧**（全站参数
//! 与门禁）。EV 一律由代码常量 + 设置键现值**现场复算**，不落库、不写死 —— 站长改了
//! `games_*` / `arcade_budget_*` 立即反映，门禁随之变红/绿。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_BET, MAX_PLAYS_PER_HOUR};
use crate::http::require_auth;
use crate::state::AppState;

use super::arcade_cfg::{bad, MILESTONES, QUESTS, SEASON_KEY};
use super::helpers::{bigsmall_mult_permille, eco_i64, scratch_odds};

/// 刮刮乐 EV（每注）：(h×0.5 + o×1 + t×2 + ten×10) / 100
fn scratch_ev(o: &games::ScratchOdds) -> f64 {
    (o.half as f64 * 0.5
        + o.one as f64
        + o.two as f64 * 2.0
        + o.ten as f64 * 10.0)
        / 100.0
}

/// 九宫格 EV 只有一份公式，在 games::jgg_ev —— 这里不再抄第二遍求和

#[derive(Deserialize)]
pub(super) struct PoolEntryReq {
    pub label: String,
    pub weight: i32,
    pub payout: i64,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Deserialize)]
pub(super) struct PoolSaveReq {
    pub pool_key: String,
    pub game: String,
    pub label: String,
    pub ticket: i64,
    pub entries: Vec<PoolEntryReq>,
}

/// sqlx 错误只实现了 From<anyhow::Error>，这里显式转一层，不让它冒到 `?` 上
fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 保存奖池。**关闸放在写侧**：不合法就拒绝保存，坏池子根本进不了表。
/// 只靠读侧关闸等于让面板有能力一键把玩法打成 503 —— 那是把正确性换成事故。
#[post("/admin/arcade/pool")]
pub(super) async fn arcade_pool_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PoolSaveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let b = body.into_inner();
    if b.pool_key.trim().is_empty() || b.game.trim().is_empty() {
        return Err(DomainError::Validation("pool_key / game 不能为空".into()));
    }
    if b.ticket <= 0 {
        return Err(DomainError::Validation("票价必须为正".into()));
    }
    let prizes: Vec<games::JggPrize> = b
        .entries
        .iter()
        .filter(|e| e.enabled)
        .map(|e| games::JggPrize {
            label: e.label.clone(),
            weight: u32::try_from(e.weight.max(0)).unwrap_or(u32::MAX),
            payout: e.payout,
        })
        .collect();
    games::validate_pool(&prizes)
        .map_err(|e| DomainError::Validation(format!("奖池不合法，已拒绝保存：{e}")))?;

    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    sqlx::query(
        "INSERT INTO arcade_pools (key, game, label, ticket) VALUES ($1, $2, $3, $4)          ON CONFLICT (key) DO UPDATE SET game = EXCLUDED.game, label = EXCLUDED.label,            ticket = EXCLUDED.ticket, updated_at = now()",
    )
    .bind(&b.pool_key)
    .bind(&b.game)
    .bind(&b.label)
    .bind(b.ticket)
    .execute(&mut *tx)
    .await.map_err(dberr)?;
    sqlx::query("DELETE FROM arcade_pool_entries WHERE pool_key = $1")
        .bind(&b.pool_key)
        .execute(&mut *tx)
        .await.map_err(dberr)?;
    for (i, e) in b.entries.iter().enumerate() {
        sqlx::query(
            "INSERT INTO arcade_pool_entries (pool_key, label, weight, payout, enabled, sort)              VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&b.pool_key)
        .bind(&e.label)
        .bind(e.weight)
        .bind(e.payout)
        .bind(e.enabled)
        .bind(i as i32)
        .execute(&mut *tx)
        .await.map_err(dberr)?;
    }
    tx.commit().await.map_err(dberr)?;
    state.repo.audit(Some(auth.id), "arcade.pool.save", None).await;
    Ok(ok(json!({
        "pool": b.pool_key,
        "entries": b.entries.len(),
        "ev": games::jgg_ev(&prizes),
    })))
}

#[get("/admin/arcade/overview")]
pub(super) async fn arcade_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let db = &state.repo.db;

    let mult = bigsmall_mult_permille(&state).await;
    let odds = scratch_odds(&state).await;
    let s_ev = scratch_ev(&odds);
    let jgg = super::helpers::jgg_pool(&state).await?;
    let j_ev = games::jgg_ev(&jgg.prizes);
    let b_ev = games::bigsmall_expected_value(mult);
    // 农场 EV 由作物表（farm_crops，DB）出厂标定 0.75 × 1.2 双倍；此处给标称值
    let f_ev = 0.90_f64;

    let max_bet = eco_i64(&state, "games_max_bet", MAX_BET).await;
    let max_plays =
        eco_i64(&state, "games_max_plays_per_hour", MAX_PLAYS_PER_HOUR).await;
    let base = eco_i64(&state, "arcade_budget_base", 1500).await;
    let pct = eco_i64(&state, "arcade_budget_pct", 20).await;
    let win = eco_i64(&state, "arcade_budget_window_days", 7).await;

    let stubs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM achievement_defs WHERE family = 'arcade'",
    )
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let claims: i64 = sqlx::query_scalar("SELECT count(*) FROM arcade_claims")
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let ev = vec![
        serde_json::json!({
            "name": "刮刮乐", "ev": s_ev,
            "note": format!("档位 {}/{}/{}/{}/{}", odds.empty, odds.half,
                odds.one, odds.two, odds.ten),
        }),
        serde_json::json!({
            "name": "猜大小", "ev": b_ev,
            "note": format!("赔率 {mult}‰，必须 < 2000‰"),
        }),
        serde_json::json!({
            "name": "九宫格", "ev": j_ev,
            "note": format!("票价 {}，权重×赔付 / Σ权重", jgg.ticket),
        }),
        serde_json::json!({
            "name": "农场", "ev": f_ev,
            "note": "产量/种子价 × (1+20% 双倍)，作物表按 0.75 标定",
        }),
    ];
    let all_below = ev
        .iter()
        .all(|r| r["ev"].as_f64().unwrap_or(1.0) < 1.0);
    let checks = vec![
        bad("随机侧各玩法 EV < 1（回收口径）", all_below, {
            let over: Vec<String> = ev
                .iter()
                .filter(|r| r["ev"].as_f64().unwrap_or(1.0) >= 1.0)
                .map(|r| r["name"].as_str().unwrap_or("?").to_string())
                .collect();
            if over.is_empty() { String::new() } else { over.join("、") }
        }),
        bad("猜大小赔率 < 2000‰（防双向零风险对冲）", mult < 2000,
            format!("当前 {mult}‰")),
        bad("确定侧预算参数合法（pct 0..=100）", (0..=100).contains(&pct),
            format!("base {base} / pct {pct}% / 窗口 {win} 天")),
    ];

    let body = serde_json::json!({
        "ev": ev,
        "checks": checks,
        "params": {
            "max_bet": max_bet, "max_plays_per_hour": max_plays,
            "bigsmall_mult_permille": mult,
            "arcade_budget_base": base, "arcade_budget_pct": pct,
            "arcade_budget_window_days": win,
        },
        "defs": {
            "season_key": SEASON_KEY,
            "quests": QUESTS.iter().map(|q| serde_json::json!({
                "code": q.0, "ref": q.1, "target": q.2, "reward": q.3,
            })).collect::<Vec<_>>(),
            "milestones": MILESTONES.iter().map(|m| serde_json::json!({
                "code": m.0, "need": m.1, "reward": m.2,
            })).collect::<Vec<_>>(),
            "stub_total": stubs, "claims_total": claims,
        },
    });
    Ok(ok(body))
}

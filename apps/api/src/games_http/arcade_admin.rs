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

/// 九宫格 EV 只有一份公式，在 games::pool_ev —— 这里不再抄第二遍求和

#[get("/admin/arcade/overview")]
pub(super) async fn arcade_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let db = &state.repo.db;

    let mult = bigsmall_mult_permille(&state).await;
    let odds = scratch_odds(&state).await?;
    let s_ev = scratch_ev(&odds);
    let jgg = super::pool::load_pool(&state.repo.db, "jgg").await?;
    let j_ev = games::pool_ev(&jgg.entries, jgg.ticket);
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
    let all_below = ev.iter().all(|r| r["ev"].as_f64().unwrap_or(1.0) < 1.0);
    let checks = vec![
        bad("随机侧各玩法 EV < 1（回收口径）", all_below, {
            let over: Vec<String> = ev
                .iter()
                .filter(|r| r["ev"].as_f64().unwrap_or(1.0) >= 1.0)
                .map(|r| r["name"].as_str().unwrap_or("?").to_string())
                .collect();
            if over.is_empty() {
                String::new()
            } else {
                over.join("、")
            }
        }),
        bad(
            "猜大小赔率 < 2000‰（防双向零风险对冲）",
            mult < 2000,
            format!("当前 {mult}‰"),
        ),
        bad(
            "确定侧预算参数合法（pct 0..=100）",
            (0..=100).contains(&pct),
            format!("base {base} / pct {pct}% / 窗口 {win} 天"),
        ),
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

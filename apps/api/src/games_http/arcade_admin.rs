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

use super::arcade_cfg::bad;
use super::arcade_rewards::{admin_rows, refs_gate, season_key};
use super::helpers::eco_i64;

/// 九宫格 EV 只有一份公式，在 games::pool_ev —— 这里不再抄第二遍求和

#[get("/admin/arcade/overview")]
pub(super) async fn arcade_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let db = &state.repo.db;

    let btable = super::pool::load_table(db, "bigsmall").await?;
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
    let mult = win_mult;
    // 刮刮乐的 EV 现在与玩法读同一张奖池行表（0248）：面板报的、闸门算的、
    // 玩法发的，是同一个数
    let scratch = super::pool::load_pool(db, "scratch").await?;
    let s_ev = games::pool_ev(&scratch.entries, scratch.ticket);
    let jgg = super::pool::load_pool(&state.repo.db, "jgg").await?;
    let j_ev = games::pool_ev(&jgg.entries, jgg.ticket);
    let b_ev = games::pool_ev(&btable.all(), btable.ticket);
    // 农场 = 确定性收获（机制常数 FARM_BASE_EV）+ 彩蛋池那一注，现值现场复算
    let fpool = super::farm_egg::load_farm(db).await?;
    let f_ev = games::farm_total_ev(&fpool.entries, fpool.unit);

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
            // 档位住在奖池行表里，note 就报行表现实：票档、档数、权重合计
            "note": format!(
                "最低注额 {}，{} 档，权重合计 {}",
                scratch.ticket,
                scratch.entries.len(),
                scratch.entries.iter().map(|e| e.weight).sum::<u32>()
            ),
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
            // 收获那一半是作物表标定的常数，彩蛋那一半住奖池行表 —— 两半都报
            "note": format!(
                "收获 {} + 彩蛋 {:.3}（{} 档，定标 {} 魔力）",
                games::FARM_BASE_EV,
                f_ev - games::FARM_BASE_EV,
                fpool.entries.len(),
                fpool.unit
            ),
        }),
    ];
    let all_below = ev.iter().all(|r| r["ev"].as_f64().unwrap_or(1.0) < 1.0);
    let mut checks = vec![
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
    // 奖励引用体检与玩家侧大厅挂同一条（名字与判据只有一份定义）：
    // 站长是在这个面板里配坏东西的，这里不照出来就等于没有。
    checks.push(refs_gate(db).await?);

    // 物品目录全表：面板要能编辑它，就得先读得到全部条目（不是只读
    // 被某个池引用的那几件）。anchor 原样回传，编辑器里保持只读。
    let item_rows = sqlx::query(
        r#"
        SELECT key, name, kind, anchor, anchor_src, unlimited,
               stock, per_user, icon, enabled, use_kind, use_ref
          FROM arcade_items ORDER BY sort, key
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<serde_json::Value> = item_rows
        .iter()
        .map(|r| {
            use sqlx::Row;
            json!({
                "key": r.get::<String, _>("key"),
                "name": r.get::<String, _>("name"),
                "kind": r.get::<String, _>("kind"),
                "anchor": r.get::<i64, _>("anchor"),
                "anchor_src": r.get::<String, _>("anchor_src"),
                "unlimited": r.get::<bool, _>("unlimited"),
                "stock": r.get::<i64, _>("stock"),
                "per_user": r.get::<i32, _>("per_user"),
                "icon": r.get::<String, _>("icon"),
                "enabled": r.get::<bool, _>("enabled"),
                "use_kind": r.get::<String, _>("use_kind"),
                "use_ref": r.get::<String, _>("use_ref"),
            })
        })
        .collect();

    let rewards = admin_rows(db).await?;
    let skey = season_key(db).await?;
    let body = serde_json::json!({
        "ev": ev,
        "checks": checks,
        "items": items,
        "params": {
            "max_bet": max_bet, "max_plays_per_hour": max_plays,
            "bigsmall_mult_permille": mult,
            "arcade_budget_base": base, "arcade_budget_pct": pct,
            "arcade_budget_window_days": win,
        },
        "defs": {
            "season_key": skey,
            "quests": rewards["quests"],
            "milestones": rewards["milestones"],
            "stub_total": stubs, "claims_total": claims,
        },
    });
    Ok(ok(body))
}

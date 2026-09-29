//! 娱乐屋「大厅游戏表面」聚合：纹章预算环 / 周常任务条 / 票根册 / 赛季星轨 / 外观货架。
//!
//! 经济口径（与 games 域一致）：随机侧（奖池）由 games.rs 的 EV<1 常量守；
//! **确定侧（周常/赛季）不进 EV 闸**，改由 arcade_budget_* 的发放预算守 ——
//! 这就是「周常送免考核卡」侧门的防线（EV 闸只管随机侧，绕得过）。
//!
//! 数据尽量派生，只有领取记录新建（arcade_claims，0240）：
//!   · 真回收 / 周常进度 ← spark_ledger(kind='game')
//!   · 票根册 ← achievement_defs(family='arcade')（见 arcade_stubs）
//!   · 外观货架 ← shop_items 化妆品类 + user_dressups

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::arcade_backpack::{backpack, det_economic_grants};
use super::arcade_board::board;
use super::arcade_cfg::{bad, COSMETIC_KINDS, MILESTONES, QUESTS, SEASON_KEY};
use super::arcade_stubs::sync_stubs;
use super::helpers::{bigsmall_mult_permille, eco_i64};

/// ISO 周键（与 arcade_claim 同口径）
const WEEK_SQL: &str = "SELECT to_char(now(), 'IYYY-\"W\"IW')";

/// 大厅游戏表面聚合（单请求，SSR 直出）。
#[get("/games/arcade-meta")]
pub(super) async fn arcade_meta(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let db = &state.repo.db;

    let base = eco_i64(&state, "arcade_budget_base", 1500).await;
    let pct = eco_i64(&state, "arcade_budget_pct", 20).await;
    let win = eco_i64(&state, "arcade_budget_window_days", 7).await;

    // 真回收：窗口内玩法流水净额（玩家净输 = 站点回收；下注为负、派彩为正）
    let back: i64 = sqlx::query_scalar(
        "SELECT COALESCE(-sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'game' \
           AND created_at >= now() - make_interval(days => $2::int)",
    )
    .bind(uid)
    .bind(win)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 确定侧发放：窗口内 arcade 奖励支出
    let det: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'arcade' AND amount > 0 \
           AND created_at >= now() - make_interval(days => $2::int)",
    )
    .bind(uid)
    .bind(win)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let budget = base + back.max(0) * win * pct / 100;
    let net = back - det;

    // ── 周常：本周（ISO 周）进度 + 领取态 ──
    let week: String = sqlx::query_scalar(WEEK_SQL)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(ref_type, ''), count(*)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'game' AND amount < 0 \
           AND created_at >= date_trunc('week', now()) \
         GROUP BY 1",
    )
    .bind(uid)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let claimed_q: Vec<(String,)> = sqlx::query_as(
        "SELECT ref_code FROM arcade_claims \
         WHERE user_id = $1 AND kind = 'quest' AND period_key = $2",
    )
    .bind(uid)
    .bind(&week)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let q_claimed: Vec<String> = claimed_q.into_iter().map(|r| r.0).collect();
    let quests: Vec<serde_json::Value> = QUESTS
        .iter()
        .map(|(code, game, target, reward)| {
            let done: i64 = if *game == "*" {
                counts.iter().map(|c| c.1).sum()
            } else {
                counts.iter().filter(|c| c.0 == *game).map(|c| c.1).sum()
            };
            let claimed = q_claimed.iter().any(|c| c == code);
            serde_json::json!({
                "code": code, "done": done.min(*target), "target": target,
                "claimed": claimed, "ready": !claimed && done >= *target,
                "reward": reward,
            })
        })
        .collect();

    // ── 票根册：实时聚合 + 达标幂等落库（见 arcade_stubs）──
    let (stubs_json, stub_count) = sync_stubs(db, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // ── 赛季星轨：里程碑按票根数解锁 + 领取态 ──
    let claimed_s: Vec<(String,)> = sqlx::query_as(
        "SELECT ref_code FROM arcade_claims \
         WHERE user_id = $1 AND kind = 'season' AND period_key = $2",
    )
    .bind(uid)
    .bind(SEASON_KEY)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let s_claimed: Vec<String> = claimed_s.into_iter().map(|r| r.0).collect();
    let season: Vec<serde_json::Value> = MILESTONES
        .iter()
        .map(|(code, need, reward)| {
            let claimed = s_claimed.iter().any(|c| c == code);
            serde_json::json!({
                "code": code, "need": need, "reward": reward,
                "claimed": claimed, "reached": stub_count >= *need,
                "ready": !claimed && stub_count >= *need,
            })
        })
        .collect();

    // ── 外观货架：shop_items 化妆品类 + 持有态 ──
    let shelf: Vec<serde_json::Value> = sqlx::query(
        "SELECT s.id, s.name, s.kind, s.price, \
                EXISTS(SELECT 1 FROM user_dressups d \
                       WHERE d.user_id = $1 AND d.item_id = s.id) AS owned \
         FROM shop_items s \
         WHERE s.kind = ANY($2) AND s.active ORDER BY s.price",
    )
    .bind(uid)
    .bind(&COSMETIC_KINDS[..])
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|r| {
        use sqlx::Row;
        serde_json::json!({
            "id": r.get::<i64, _>("id"),
            "name": r.get::<String, _>("name"),
            "kind": r.get::<String, _>("kind"),
            "price": r.get::<i64, _>("price"),
            "owned": r.get::<bool, _>("owned"),
            "zero_debt": true,
        })
    })
    .collect();

    // ── 全服公示：近期票根/领取事件（社交钩子；文案由前端 i18n 合成）──
    let feed: Vec<serde_json::Value> = sqlx::query(
        "SELECT x.at, u.username AS who, x.kind, x.name FROM ( \
           SELECT ua.granted_at AS at, ua.user_id, 'stub' AS kind, \
                  d.name AS name \
           FROM user_achievements ua \
           JOIN achievement_defs d ON d.id = ua.def_id \
           WHERE d.family = 'arcade' \
           UNION ALL \
           SELECT c.claimed_at AS at, c.user_id, c.kind, c.ref_code AS name \
           FROM arcade_claims c \
         ) x JOIN users u ON u.id = x.user_id \
         WHERE u.status < 2 \
         ORDER BY x.at DESC LIMIT 12",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|r| {
        use sqlx::Row;
        serde_json::json!({
            "who": r.get::<String, _>("who"),
            "kind": r.get::<String, _>("kind"),
            "name": r.get::<String, _>("name"),
        })
    })
    .collect();

    // 背包：奖池发出的物品必须回到玩家眼前，否则「发奖」只是账面上的一行
    let pack = backpack(db, uid).await?;
    let board = board(db).await?;
    let det_items = det_economic_grants(db, win).await?;

    // ── 门禁：确定侧预算（第二道闸）+ 随机侧赔率方向 ──
    let mult = bigsmall_mult_permille(&state).await;
    let checks = vec![
        bad(
            "确定侧发放在娱乐屋预算内",
            det <= budget,
            format!("已发 {det} / 预算 {budget}"),
        ),
        bad(
            "猜大小赔率 < 2.0（随机侧 EV < 1）",
            mult < 2000,
            format!("当前 {mult}‰"),
        ),
        bad(
            "确定侧未发放经济类物品（EV 闸管不到这一侧）",
            det_items == 0,
            format!("窗口内 {det_items} 笔"),
        ),
    ];

    let body = serde_json::json!({
        "ledger": {
            "magic_back": back, "det_cost": det, "net": net,
            "budget": budget, "base": base, "pct": pct, "window_days": win,
        },
        "quests": { "period": week, "items": quests },
        "stubs": {
            "owned": stub_count, "total": stubs_json.len(), "items": stubs_json,
        },
        "season": { "key": SEASON_KEY, "items": season },
        "shelf": shelf,
        "backpack": pack,
        "board": board,
        "feed": feed,
        "checks": checks,
    });
    Ok(ok(body))
}

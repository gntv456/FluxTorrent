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

use super::arcade_backpack::backpack;
use super::arcade_board::board;
use super::arcade_cfg::{bad, COSMETIC_KINDS};
use super::arcade_rewards::{
    det_cost_value, load_milestones, load_quests, refs_gate, season_key,
};
use super::arcade_stubs::sync_stubs;
use super::helpers::eco_i64;
use super::pool::load_table;

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

    // 确定侧发放：魔力按流水计，物品按目录 anchor 计 —— 两边合起来才是
    // 「这一周站点确定付出了多少」。只数魔力等于给确定侧留了一条不进账的路。
    let det_spark: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'arcade' AND amount > 0 \
           AND created_at >= now() - make_interval(days => $2::int)",
    )
    .bind(uid)
    .bind(win)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let det_item_value = det_cost_value(db, uid, win).await?;
    let det = det_spark + det_item_value;

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
    let quest_rows = load_quests(db).await?;
    let quests: Vec<serde_json::Value> = quest_rows
        .iter()
        .map(|r| {
            let done: i64 = if r.game_ref == "*" {
                counts.iter().map(|c| c.1).sum()
            } else {
                counts
                    .iter()
                    .filter(|c| c.0 == r.game_ref)
                    .map(|c| c.1)
                    .sum()
            };
            let claimed = q_claimed.iter().any(|c| c == &r.code);
            serde_json::json!({
                "code": r.code,
                "done": done.min(r.target),
                "target": r.target,
                "claimed": claimed,
                "ready": !claimed && done >= r.target,
                "reward": r.reward_spark,
                "item_key": r.item_key,
                "item_name": r.item_name,
                "item_qty": r.item_qty,
            })
        })
        .collect();

    // ── 票根册：实时聚合 + 达标幂等落库（见 arcade_stubs）──
    let (stubs_json, stub_count) = sync_stubs(db, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // ── 赛季星轨：里程碑按票根数解锁 + 领取态 ──
    let skey = season_key(db).await?;
    let claimed_s: Vec<(String,)> = sqlx::query_as(
        "SELECT ref_code FROM arcade_claims \
         WHERE user_id = $1 AND kind = 'season' AND period_key = $2",
    )
    .bind(uid)
    .bind(&skey)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let s_claimed: Vec<String> = claimed_s.into_iter().map(|r| r.0).collect();
    let season: Vec<serde_json::Value> = load_milestones(db, &skey)
        .await?
        .iter()
        .map(|r| {
            let claimed = s_claimed.iter().any(|c| c == &r.code);
            serde_json::json!({
                "code": r.code, "need": r.target,
                "reward": r.reward_spark,
                "item_key": r.item_key, "item_name": r.item_name,
                "item_qty": r.item_qty,
                "claimed": claimed, "reached": stub_count >= r.target,
                "ready": !claimed && stub_count >= r.target,
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

    // ── 全服公示：近期票根/领取/稀有掉落事件（社交钩子；文案由前端 i18n 合成）──
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
           UNION ALL \
           SELECT r.created_at AS at, r.user_id, 'rare' AS kind, \
                  r.game || '·' || r.prize AS name \
           FROM arcade_pool_rounds r WHERE r.rarity >= 4 \
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
    let refs = refs_gate(db).await?;

    // ── 门禁：确定侧预算（第二道闸）+ 随机侧赔率方向 ──
    let btable = load_table(db, "bigsmall").await?;
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
    // 农场土地阶梯：五个键都是真设置键（0260 的 settings_meta），配错会让
    // 农场整页读不出来 —— 报价口径与「参数现值」由 land_gate_row 一并报。
    let land = super::farm_land::land_gate_row(&state).await;
    let checks = vec![
        bad(
            "确定侧发放（魔力 + 物品折算）落在娱乐屋预算内",
            det <= budget,
            format!(
                "已发 {det}（魔力 {det_spark} + 物品 {det_item_value}）\
                 / 预算 {budget}"
            ),
        ),
        bad(
            "猜大小赔率 < 2.0（随机侧 EV < 1）",
            mult < 2000,
            format!("当前 {mult}‰"),
        ),
        // 停用一件正被奖励引用的物品是普通运营动作，而确定侧没有「打折回落」：
        // 引用坏掉时玩家点领取会当场报错。这条闸就是提前把它照出来。
        refs,
        land,
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
        "season": { "key": skey, "items": season },
        "shelf": shelf,
        "backpack": pack,
        "board": board,
        "feed": feed,
        "checks": checks,
    });
    Ok(ok(body))
}

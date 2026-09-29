//! 玩家侧物品背包（只读发放账 arcade_item_grants）。
//!
//! 奖池能发物品之后，「发出去的东西玩家看得见吗」是闭环的另一半。这里刻意
//! **不另存一份持有数**：余量与持有量一律由发放账反推（第二份清单必然漂移），
//! 与 0244 的 arcade_item_stock_left 视图同一口径。
//!
//! det_economic_grants 是给运营面板用的实测量：确定侧（周常/赛季）不进 EV 闸，
//! 一旦有人给它挂上经济类物品，面板必须真的变红，而不是写死一句「不含」。

use serde_json::json;
use sqlx::{PgPool, Row};

use super::pool::dberr;
use crate::errors::DomainResult;

/// 我的背包：按物品聚合的持有件数 + 最近一次来源玩法。
pub(super) async fn backpack(
    db: &PgPool,
    uid: i64,
) -> DomainResult<serde_json::Value> {
    let rows = sqlx::query(
        r#"
        SELECT i.key, i.name, i.icon, i.kind, i.anchor, i.anchor_src,
               i.enabled,
               SUM(g.qty)::bigint              AS qty,
               MAX(g.granted_at)               AS last_at,
               (ARRAY_AGG(g.game
                  ORDER BY g.granted_at DESC))[1] AS last_game
          FROM arcade_item_grants g
          JOIN arcade_items i ON i.key = g.item_key
         WHERE g.user_id = $1
         GROUP BY i.key, i.name, i.icon, i.kind, i.anchor,
                  i.anchor_src, i.enabled
         ORDER BY MAX(g.granted_at) DESC
        "#,
    )
    .bind(uid)
    .fetch_all(db)
    .await
    .map_err(dberr)?;

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "key": r.get::<String, _>("key"),
                "name": r.get::<String, _>("name"),
                "icon": r.get::<String, _>("icon"),
                "kind": r.get::<String, _>("kind"),
                "anchor": r.get::<i64, _>("anchor"),
                "anchor_src": r.get::<String, _>("anchor_src"),
                "enabled": r.get::<bool, _>("enabled"),
                "qty": r.get::<i64, _>("qty"),
                "last_game": r.get::<Option<String>, _>("last_game"),
                "last_at": r
                    .get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_at")
                    .map(|t| t.to_rfc3339()),
            })
        })
        .collect();

    let total: i64 = items
        .iter()
        .filter_map(|x| x.get("qty").and_then(|q| q.as_i64()))
        .sum();
    Ok(json!({ "total": total, "kinds": items.len(), "items": items }))
}

/// 窗口内「确定侧」发出的经济类物品笔数——应当恒为 0，EV 闸管不到这一侧。
pub(super) async fn det_economic_grants(
    db: &PgPool,
    window_days: i64,
) -> DomainResult<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)::bigint
          FROM arcade_item_grants g
          JOIN arcade_items i ON i.key = g.item_key
         WHERE g.side = 'det' AND i.kind = 'economic'
           AND g.granted_at >= now() - make_interval(days => $1::int)
        "#,
    )
    .bind(window_days as i32)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(n)
}

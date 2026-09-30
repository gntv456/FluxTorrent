//! 玩家侧物品背包（读侧）。
//!
//! 奖池能发物品之后，「发出去的东西玩家看得见吗」是闭环的另一半。这里刻意
//! **不另存一份持有数**：余量与持有量一律由两张账反推（第二份清单必然漂移），
//! 与 0244 的 arcade_item_stock_left 视图同一口径。

use serde_json::json;
use sqlx::{PgPool, Row};

use super::pool::dberr;
use crate::errors::DomainResult;

/// 我的背包：按物品聚合的**持有**件数（发放 − 消耗）+ 用途 + 最近来源玩法。
///
/// 0246 加了消耗账之后，`qty` 语义从「发过多少」变成「还能用多少」——
/// 前台拿它显示数量、后端拿它判能不能扣，两处必须是同一个数，所以直接读
/// arcade_item_held 视图而不是在这里再减一遍。
pub(super) async fn backpack(
    db: &PgPool,
    uid: i64,
) -> DomainResult<serde_json::Value> {
    let rows = sqlx::query(
        r#"
        SELECT i.key, i.name, i.icon, i.kind, i.anchor, i.anchor_src,
               i.enabled, i.use_kind, i.use_ref,
               h.held, h.granted, h.used,
               gr.last_at, gr.last_game
          FROM arcade_item_held h
          JOIN arcade_items i ON i.key = h.item_key
          LEFT JOIN (
                SELECT item_key, MAX(granted_at) AS last_at,
                       (ARRAY_AGG(game ORDER BY granted_at DESC))[1]
                         AS last_game
                  FROM arcade_item_grants
                 WHERE user_id = $1
                 GROUP BY item_key
          ) gr ON gr.item_key = h.item_key
         WHERE h.user_id = $1
         ORDER BY gr.last_at DESC NULLS LAST
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
                "use_kind": r.get::<String, _>("use_kind"),
                "use_ref": r.get::<String, _>("use_ref"),
                // qty 对外语义 = 还能用几件（发放 − 消耗），与后端扣减判定同一个数
                "qty": r.get::<i64, _>("held"),
                "granted": r.get::<i64, _>("granted"),
                "used": r.get::<i64, _>("used"),
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

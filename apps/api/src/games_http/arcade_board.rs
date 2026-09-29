//! 娱乐屋周榜（L3 展示层）：票根收集榜 + 本周活跃榜。
//!
//! 口径是这条线定死的一条：**榜单只按收集度与局数排，绝不按净输赢排**——
//! 拿净输赢排榜等于公开鼓励玩家加注，与「四玩法一律回收魔力」对着干。
//! 两个榜都从既有事实表反推，不新建计数表：票根 ← user_achievements(family
//! ='arcade')，局数 ← spark_ledger(kind='game')。

use serde_json::{json, Value};
use sqlx::{PgPool, Row};

use super::pool::dberr;
use crate::errors::DomainResult;

/// 票根收集榜：持有数并列时按最早点亮时间排（先做到的人在前）。
async fn stub_board(db: &PgPool) -> DomainResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT u.username,
               count(*)::bigint   AS stubs,
               min(ua.granted_at) AS first_at
          FROM user_achievements ua
          JOIN achievement_defs d ON d.id = ua.def_id
          JOIN users u ON u.id = ua.user_id
         WHERE d.family = 'arcade' AND u.status < 2
         GROUP BY u.id, u.username
         ORDER BY stubs DESC, min(ua.granted_at) ASC
         LIMIT 8
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(dberr)?;

    Ok(rows
        .into_iter()
        .map(|r| {
            json!({
                "who": r.get::<String, _>("username"),
                "n": r.get::<i64, _>("stubs"),
            })
        })
        .collect())
}

/// 本周活跃榜：自然周（Postgres 的 date_trunc('week') 以周一为界）玩法局数。
async fn play_board(db: &PgPool) -> DomainResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT u.username, count(*)::bigint AS plays
          FROM spark_ledger l
          JOIN users u ON u.id = l.user_id
         WHERE l.kind = 'game' AND u.status < 2
           AND l.created_at >= date_trunc('week', now())
         GROUP BY u.id, u.username
         ORDER BY plays DESC, max(l.created_at) ASC
         LIMIT 8
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(dberr)?;

    Ok(rows
        .into_iter()
        .map(|r| {
            json!({
                "who": r.get::<String, _>("username"),
                "n": r.get::<i64, _>("plays"),
            })
        })
        .collect())
}

pub(super) async fn board(db: &PgPool) -> DomainResult<Value> {
    Ok(json!({
        "stubs": stub_board(db).await?,
        "plays": play_board(db).await?,
    }))
}

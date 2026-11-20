//! 娱乐屋周榜派奖（0503 运营台账 #4）：欧皇/渔王上周 TOP3 自动发火花。
//!
//! 周榜此前「只读不派奖」（arcade_leaderboard.rs:4 自注）——展示资产
//! 未变现为留存钩子。本 job 每日跑：检查**上一个完整周**是否已派
//! （spark_ledger 幂等键 arcade-week-prize:{kind}:{week}:* 存在即跳过），
//! 未派则按 site_settings arcade_week_prize_1/2/3（缺省 300/200/100，
//! 0=关）向上周 TOP3 发奖 + PM。窗口与周榜同口径（上海时区自然周），
//! 派的是「上周完整榜」——周中跑不重复、不漏发。

use sqlx::PgPool;

const WEEK_SQL: &str = "SELECT to_char(date_trunc('week', \
     (now() AT TIME ZONE 'Asia/Shanghai') - interval '7 days') \
     AT TIME ZONE 'Asia/Shanghai', 'IYYY-\"W\"IW')";

/// 0507 品类泛化（运营体检三批）：0505 的开关只能启停 lucky/fishing
/// 两条专用 SQL，加新品类仍要改代码。现把「kind → 榜单 SQL 片段」
/// 注册表化（arcade_board_kinds 表，0507 迁移播种 lucky/fishing 两行
/// 保持行为不变）：加新品类 = INSERT 一行（label + sql 片段，{week}
/// 占位窗口）。片断只接受注册表内固定行（无后台 CRUD 口），无注入面。
/// site_settings.arcade_week_prize_kinds 仍作总开关（空=全关）。
async fn prize_kinds(db: &PgPool) -> Vec<(String, String, String)> {
    let switch: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings \
         WHERE name = 'arcade_week_prize_kinds'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let enabled: Vec<String> = switch
        .as_deref()
        .unwrap_or("lucky,fishing")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    sqlx::query_as(
        "SELECT kind, label, board_sql FROM arcade_board_kinds \
         ORDER BY kind",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|(k, _, _)| enabled.contains(k))
    .collect()
}

pub(crate) async fn arcade_week_prize(db: &PgPool) -> anyhow::Result<u64> {
    let week: String = sqlx::query_scalar(WEEK_SQL)
        .fetch_one(db)
        .await
        .unwrap_or_default();
    if week.is_empty() {
        return Ok(0);
    }
    let prizes: (i64, i64, i64) = sqlx::query_as(
        "SELECT \\
           COALESCE((SELECT value::bigint FROM site_settings \\
             WHERE name = 'arcade_week_prize_1'), 300), \\
           COALESCE((SELECT value::bigint FROM site_settings \\
             WHERE name = 'arcade_week_prize_2'), 200), \\
           COALESCE((SELECT value::bigint FROM site_settings \\
             WHERE name = 'arcade_week_prize_3'), 100)",
    )
    .fetch_one(db)
    .await
    .unwrap_or((300, 200, 100));
    if prizes.0 <= 0 {
        return Ok(0);
    }
    let mut acted = 0u64;
    let kinds = prize_kinds(db).await;
    for (kind, label, board_sql) in &kinds {
        // 榜单 SQL 由注册表行提供（{week} 占位替换为周锚字符串）
        let sql = board_sql.replace("{week}", &week);
        let winners: Vec<(i64, i64)> =
            sqlx::query_as(&sql).fetch_all(db).await.unwrap_or_default();
        for (i, (uid, v)) in winners.iter().enumerate() {
            let spark = [prizes.0, prizes.1, prizes.2][i.min(2)];
            if spark <= 0 {
                continue;
            }
            let idem = format!("arcade-week-prize:{kind}:{week}:{uid}");
            let mut tx = match db.begin().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(?e, kind, %week, "周榜派奖事务失败");
                    continue;
                }
            };
            let credited = match sqlx::query(
                "INSERT INTO spark_ledger \\
                    (id, user_id, amount, kind, idempotency_key, \\
                     balance_after) \\
                  SELECT nextval('spark_ledger_id_seq'), $1, $2, \\
                    'arcade_week', $3, u.spark_balance + $2 \\
                  FROM users u WHERE u.id = $1 \\
                  AND NOT EXISTS (SELECT 1 FROM spark_ledger \\
                                  WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(spark)
            .bind(&idem)
            .execute(&mut *tx)
            .await
            {
                Ok(r) => r.rows_affected() > 0,
                Err(e) => {
                    tracing::warn!(?e, kind, %week, uid, "周榜奖励流水失败");
                    let _ = tx.rollback().await;
                    continue;
                }
            };
            if credited {
                if let Err(e) = sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2 \\
                     WHERE id = $1",
                )
                .bind(uid)
                .bind(spark)
                .execute(&mut *tx)
                .await
                {
                    tracing::warn!(?e, kind, %week, uid, "周榜余额更新失败");
                    let _ = tx.rollback().await;
                    continue;
                }
                let _ = sqlx::query(
                    "INSERT INTO messages \\
                     (sender_id, receiver_id, subject, body) \\
                     VALUES (NULL, $1, $2, $3)",
                )
                .bind(uid)
                .bind(format!("{label}周榜 TOP{}", i + 1))
                .bind(format!(
                    "上周娱乐屋{label}榜你排第 {}（值 {v}），\\
                     周榜奖励 {spark} 魔力已入账。本周榜单已重置——继续冲！",
                    i + 1
                ))
                .execute(&mut *tx)
                .await;
            }
            let _ = tx.commit().await;
            acted += 1;
        }
    }
    Ok(acted)
}

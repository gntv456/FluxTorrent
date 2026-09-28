//! 分区维护/休眠标记/运行日志清理。
//! 从 jobs.rs 按域拆出。

use chrono::Datelike;
use sqlx::PgPool;

/// 运行日志保留期（0218 G6）：14 天 + 20 万行硬顶（取严，防告警风暴撑爆库）。
/// 小时级跑；DELETE 走 ts 索引与主键，代价可忽略。
pub(crate) async fn purge_runtime_logs(db: &PgPool) -> anyhow::Result<u64> {
    let a = sqlx::query(
        "DELETE FROM runtime_logs WHERE ts < now() - interval '14 days'",
    )
    .execute(db)
    .await?
    .rows_affected();
    let b = sqlx::query(
        "DELETE FROM runtime_logs WHERE id < \
         (SELECT COALESCE(max(id), 0) FROM runtime_logs) - 200000",
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(a + b)
}

/// P0-2 分区预建：为三张 RANGE 流水表预建 [当月, +2 月] 的月分区（每日一次，IF NOT EXISTS 幂等）。
/// 存量拆分见迁移 0070；没有本 job 时数据会持续落 default 分区导致裁剪失效。
/// 0230 G31-D4：同时按 site_settings.ledger_retain_months（0=永久，缺省）
/// 处置过期分区——先 DETACH（查询立即不再命中、瞬间完成），被 DETACH 的
/// 孤表下一个周期再 DROP（给误配窗口留缓冲）。
pub async fn ensure_partitions(db: &PgPool) -> anyhow::Result<()> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let first_of_month = now_site.date_naive().with_day(1).unwrap();
    let tables = [
        ("traffic_ledger", "window_start"),
        ("spark_ledger", "created_at"),
    ];
    for i in 0..3i32 {
        let start = first_of_month
            .checked_add_months(chrono::Months::new(i as u32))
            .unwrap_or(first_of_month);
        let end = start
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(start);
        // posts（论坛正文）分区预建保留（不预建新帖会落 default），但绝不进
        // 归档清单——它是业务数据不是流水
        let mut all: Vec<&str> = tables.iter().map(|(t, _)| *t).collect();
        all.push("posts");
        for tbl in all {
            let name = format!("{}_{}", tbl, start.format("%Y_%m"));
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} \
                 FOR VALUES FROM ('{}') TO ('{}')",
                name,
                tbl,
                start.format("%Y-%m-%d"),
                end.format("%Y-%m-%d")
            ))
            .execute(db)
            .await?;
        }
    }
    drop_expired_partitions(db).await?;
    Ok(())
}

/// 过期分区处置（0230 G31-D4）：retain > 0 时，分区上界早于
/// (当月 - retain) 的 DETACH；已 DETACH 的同名前缀孤表（上轮遗留）DROP。
/// 两步走的意义：DETACH 秒级且可逆（误配时 ATTACH 回来即可），DROP
/// 延后一个周期执行。
async fn drop_expired_partitions(db: &PgPool) -> anyhow::Result<()> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings \
         WHERE name = 'ledger_retain_months'",
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);
    let retain: i64 = raw.and_then(|v| v.parse().ok()).unwrap_or(0);
    if retain <= 0 {
        return Ok(()); // 永久保留（缺省，兼容现状）
    }
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let Some(cutoff) = now_site
        .date_naive()
        .checked_sub_months(chrono::Months::new(retain as u32))
    else {
        return Ok(());
    };
    // ⚠️ posts（论坛正文）是业务数据不是流水——绝不进归档清单（P0 审查修正：
    // 它同是 RANGE 分区表故曾被一并圈入，retention 会删帖）
    let prefixes = ["traffic_ledger_", "spark_ledger_"];
    for prefix in prefixes {
        // 在线分区：查 pg_inherits 拿分区上界（relname 编码了年月）
        let parts: Vec<(String,)> = sqlx::query_as(&format!(
            "SELECT c.relname FROM pg_class c \
             JOIN pg_inherits i ON i.inhrelid = c.oid \
             JOIN pg_class p ON p.oid = i.inhparent \
             WHERE p.relname = '{}' AND c.relname LIKE '{}%'",
            prefix.trim_end_matches('_'),
            prefix
        ))
        .fetch_all(db)
        .await?;
        for (name,) in parts {
            // relname 形如 traffic_ledger_2024_06 → 上界 = 2024-07-01
            let Some(bound) = name
                .rsplit('_')
                .next()
                .and_then(|m| m.parse::<u32>().ok())
                .and_then(|m| {
                    name.rsplitn(2, '_')
                        .nth(1)?
                        .parse::<i32>()
                        .ok()
                        .map(|y| (y, m))
                })
            else {
                continue;
            };
            let upper = chrono::NaiveDate::from_ymd_opt(bound.0, bound.1, 1)
                .and_then(|d| d.checked_add_months(chrono::Months::new(1)));
            if let Some(upper) = upper {
                if upper < cutoff {
                    sqlx::query(&format!(
                        "ALTER TABLE {} DETACH PARTITION {}",
                        prefix.trim_end_matches('_'),
                        name
                    ))
                    .execute(db)
                    .await?;
                    tracing::info!(%name, "流水分区超保留期，已 DETACH（下周期 DROP）");
                }
            }
        }
        // 上轮 DETACH 的孤表：DROP（普通表，不在继承关系里）
        let orphans: Vec<(String,)> = sqlx::query_as(&format!(
            "SELECT relname FROM pg_class \
             WHERE relname LIKE '{}%' AND relkind = 'r' \
               AND NOT EXISTS (SELECT 1 FROM pg_inherits \
                     WHERE inhrelid = pg_class.oid)",
            prefix
        ))
        .fetch_all(db)
        .await?;
        for (name,) in orphans {
            sqlx::query(&format!("DROP TABLE IF EXISTS {}", name))
                .execute(db)
                .await?;
            tracing::info!(%name, "已 DROP 过期流水分区");
        }
    }
    Ok(())
}

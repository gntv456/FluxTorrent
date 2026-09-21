//! 分区维护/休眠标记。
//! 从 jobs.rs 按域拆出。

use chrono::Datelike;
use sqlx::PgPool;

/// P0-2 分区预建：为三张 RANGE 流水表预建 [当月, +2 月] 的月分区（每日一次，IF NOT EXISTS 幂等）。
/// 存量拆分见迁移 0070；没有本 job 时数据会持续落 default 分区导致裁剪失效。
pub async fn ensure_partitions(db: &PgPool) -> anyhow::Result<()> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let first_of_month = now_site.date_naive().with_day(1).unwrap();
    let tables = [
        ("traffic_ledger", "window_start"),
        ("spark_ledger", "created_at"),
        ("posts", "created_at"),
    ];
    for (tbl, col) in tables {
        for i in 0..3i32 {
            let start = first_of_month + chrono::Duration::days(30 * i as i64);
            // 用 date_trunc 语义对齐月首（+30 天近似在月末附近可能漂移，改为逐次取下月一号）
            let start = first_of_month
                .checked_add_months(chrono::Months::new(i as u32))
                .unwrap_or(start);
            let end = start
                .checked_add_months(chrono::Months::new(1))
                .unwrap_or(start);
            let name = format!("{}_{}", tbl, start.format("%Y_%m"));
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM ('{}') TO ('{}')",
                name,
                tbl,
                start.format("%Y-%m-%d"),
                end.format("%Y-%m-%d")
            ))
            .execute(db)
            .await?;
            let _ = col; // 分区键仅作文档提示
        }
    }
    Ok(())
}

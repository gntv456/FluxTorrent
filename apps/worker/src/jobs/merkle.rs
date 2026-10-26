//! 每日火花流水 Merkle 封存（Asia/Shanghai 日界）。
//!
//! 封的是**前一日**：当天流水还在写，封了也不完整。回补最近
//! BACKFILL_DAYS 天，覆盖 worker 短暂停机；已封存的日子永不重算，
//! 否则「事后改账」就又能把树根一起改掉，凭证失去意义。
//!
//! 调度不在 run.rs 的 tick 组里：run.rs 已经是存量超限文件（只许瘦），
//! 本任务自带 60s 循环，经 main.rs 起一个独立 task；锁/登记仍走
//! with_lock，与定时任务共用同一把 advisory lock 与 job_status 面板。

use chrono::{DateTime, Duration, NaiveDate, Utc};
use flux_shared::merkle::{self, LedgerLeaf};
use sqlx::PgPool;

/// 首次上线/停机后的回补窗口（天）。
const BACKFILL_DAYS: i64 = 7;

fn site_day(now: DateTime<Utc>) -> NaiveDate {
    (now + Duration::hours(8)).date_naive()
}

/// 站点日 [00:00, 24:00) 对应的 UTC 区间。
fn bounds(day: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = day.and_hms_opt(0, 0, 0).expect("midnight 必然合法")
        - Duration::hours(8);
    let end = start + Duration::days(1);
    (
        DateTime::from_naive_utc_and_offset(start, Utc),
        DateTime::from_naive_utc_and_offset(end, Utc),
    )
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    user_id: i64,
    amount: i64,
    kind: String,
    ref_type: Option<String>,
    ref_id: Option<i64>,
    balance_after: Option<i64>,
    created: String,
}

pub(crate) async fn spark_merkle_daily(db: &PgPool) -> anyhow::Result<u64> {
    let today = site_day(Utc::now());
    let mut built = 0;
    for back in 1..=BACKFILL_DAYS {
        built += seal_day(db, today - Duration::days(back)).await?;
    }
    Ok(built)
}

async fn seal_day(db: &PgPool, day: NaiveDate) -> anyhow::Result<u64> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_merkle_days WHERE day = $1)",
    )
    .bind(day)
    .fetch_one(db)
    .await?;
    if exists {
        return Ok(0);
    }

    let (start, end) = bounds(day);
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, user_id, amount, kind, ref_type, ref_id, balance_after, \
           to_char(created_at AT TIME ZONE 'UTC', \
                   'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created \
         FROM spark_ledger \
         WHERE created_at >= $1 AND created_at < $2 \
         ORDER BY created_at, user_id, id",
    )
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await?;

    let day_s = day.format("%Y-%m-%d").to_string();
    let mut leaves = Vec::with_capacity(rows.len());
    let mut preimages = Vec::with_capacity(rows.len());
    for r in &rows {
        let pre = merkle::ledger_preimage(&LedgerLeaf {
            day: &day_s,
            id: r.id,
            user_id: r.user_id,
            amount: r.amount,
            kind: &r.kind,
            ref_type: r.ref_type.as_deref(),
            ref_id: r.ref_id,
            balance_after: r.balance_after,
            created_at: &r.created,
        });
        leaves.push(merkle::leaf_hash(&pre));
        preimages.push(pre);
    }
    let root = merkle::root(&leaves)
        .ok_or_else(|| anyhow::anyhow!("叶子哈希不是 64 位 hex"))?;

    let users: std::collections::BTreeSet<i64> =
        rows.iter().map(|r| r.user_id).collect();
    let idxs: Vec<i64> = (0..rows.len() as i64).collect();
    let uids: Vec<i64> = rows.iter().map(|r| r.user_id).collect();
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let amounts: Vec<i64> = rows.iter().map(|r| r.amount).collect();
    let kinds: Vec<String> = rows.iter().map(|r| r.kind.clone()).collect();
    let created: Vec<String> = rows.iter().map(|r| r.created.clone()).collect();

    let mut tx = db.begin().await?;
    let ins = sqlx::query(
        "INSERT INTO spark_merkle_days (day, root, leaf_count, user_count) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (day) DO NOTHING",
    )
    .bind(day)
    .bind(&root)
    .bind(leaves.len() as i64)
    .bind(users.len() as i64)
    .execute(&mut *tx)
    .await?;
    if ins.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(0);
    }
    if !leaves.is_empty() {
        sqlx::query(
            "INSERT INTO spark_merkle_leaves \
               (day, idx, user_id, ledger_id, amount, kind, created, \
                leaf, preimage) \
             SELECT $1, * FROM unnest( \
               $2::bigint[], $3::bigint[], $4::bigint[], $5::bigint[], \
               $6::text[], $7::text[], $8::text[], $9::text[])",
        )
        .bind(day)
        .bind(&idxs)
        .bind(&uids)
        .bind(&ids)
        .bind(&amounts)
        .bind(&kinds)
        .bind(&created)
        .bind(&leaves)
        .bind(&preimages)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(
        day = %day,
        leaves = leaves.len(),
        users = users.len(),
        "spark_merkle sealed"
    );
    Ok(1)
}

/// 独立调度循环（60s）：封存是幂等的，跨日/重启都会自然补跑。
pub(crate) async fn spark_merkle_loop(db: PgPool) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        if crate::shutdown::shutting_down() {
            break;
        }
        let d = db.clone();
        let _ = super::locks::with_lock(&db, "job:spark_merkle", async move {
            spark_merkle_daily(&d).await
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_day_flips_at_utc_1600() {
        let before = "2026-10-20T15:59:59Z".parse::<DateTime<Utc>>().unwrap();
        let after = "2026-10-20T16:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(site_day(before).to_string(), "2026-10-20");
        assert_eq!(site_day(after).to_string(), "2026-10-21");
    }

    #[test]
    fn day_bounds_are_utc_16_to_16() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let (start, end) = bounds(day);
        assert_eq!(start.to_rfc3339(), "2026-10-19T16:00:00+00:00");
        assert_eq!(end.to_rfc3339(), "2026-10-20T16:00:00+00:00");
    }
}

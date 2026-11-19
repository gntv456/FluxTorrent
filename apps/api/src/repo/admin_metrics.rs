//! 管理大盘的北极星取数（H 层禁 SQL：查询集中住在 repo）。
//! MASU = 30 天内仍有做种 announce 的去重用户数；MAU = 同期活跃用户数。

use sqlx::PgPool;

/// 月活做种用户（MASU，北极星）。snatches.last_seen_at 有部分索引。
pub async fn masu(db: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(DISTINCT user_id) FROM snatches \
         WHERE seeding AND last_seen_at > now() - interval '30 days'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

/// 月活用户（MAU）：MASU 的分母。
pub async fn mau(db: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM users \
         WHERE last_seen_at > now() - interval '30 days'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

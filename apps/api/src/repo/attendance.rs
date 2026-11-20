//! 签到域的 SQL 唯一居所（FRAME §2：H 层禁 sqlx，SQL 只住 repo/）。
//!
//! 背景（2026-11-20 五问评审 P0）：`GET /attendance` 的响应体里**没有**
//! `total_days`，前端只能拿 `recent.length`（近 30 天）兜底，于是连签
//! 483 天的老玩家在签到页看到的是「累计签到 30」——同一个指标在首页
//! （home_attendance.rs 用全量行数）是对的、在签到页是错的。
//!
//! 这里补一条全量计数，H 层只调函数（`economy_http/checkin/status.rs`）。
//! 用主库而非只读副本：签到写后立读要最新值，副本延迟会让当日签到数倒退。

use super::Repo;

/// 本人**累计**签到天数（全量口径，含补签行）。
///
/// 与首页「累计签到」同源（`apps/api/src/http/home_attendance.rs` 取
/// 全量行数），与 `/attendance` 的 `recent`（近 30 天）口径不同——
/// 两者不要再混用。
pub async fn total_days(repo: &Repo, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM attendance WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&repo.db)
        .await
        .unwrap_or(0)
}

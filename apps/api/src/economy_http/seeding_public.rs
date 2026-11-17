//! 做种收益预估的对外口径（供开放 API / 插件用户卡复用）。
//!
//! 为什么单独一层薄壳：预估本体在 `seeding_est`（SQL 在那里，与 worker 结算
//! 逐条对齐），而开放 API 与插件兼容层需要同一口径的**时魔**数值。
//! 薄壳只做两件事：算僵尸阈值（读设置）+ 转调预估函数——不碰 SQL，
//! 避免同一口径在两处各写一遍（写了就必然漂移）。

use crate::repo::settings;
use crate::repo::Repo;

use super::seeding_est::{hourly_estimate, hourly_points_estimate};

/// 僵尸阈值：max(2h, 2×announce_interval) —— 与 worker 的 stale_peer_threshold_secs 同源。
///
/// announce_interval 缺失/非法时回落 1800 秒（worker 缺省同值）。
pub async fn stale_threshold_secs(repo: &Repo) -> i64 {
    let announce: i64 = settings::read(repo, "announce_interval")
        .await
        .map(|v| v.trim().to_string())
        .and_then(|v| v.parse::<i64>().ok())
        .map(|v| v.clamp(60, 86400))
        .unwrap_or(1800);
    (announce * 2).max(7200)
}

/// 时魔：本人下一个小时的做种收益预估（火花/小时）。
///
/// 口径与 worker 结算同源（`seeding_hourly`），因此页面「我的魔力」与
/// 插件用户卡显示的时魔是同一个数，不会出现两处对不上。
pub async fn bonus_per_hour(repo: &Repo, uid: i64) -> i64 {
    let stale = stale_threshold_secs(repo).await;
    hourly_estimate(&repo.db, uid, stale).await
}

/// 时积分：下一个小时预计累计的做种积分（做种小时数口径，与收益闸门不同）。
pub async fn bonus_points_per_hour(repo: &Repo, uid: i64) -> i64 {
    let stale = stale_threshold_secs(repo).await;
    hourly_points_estimate(&repo.db, uid, stale).await
}

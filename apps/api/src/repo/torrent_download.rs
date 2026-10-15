//! `.torrent` 构建链的数据访问（`publish_http/download.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」。
//!
//! 五处查询 + 两处设置读，逐字搬运：
//!   ① 种名（P-299 存盘名 Content-Disposition）
//!   ② torrent_files 原文（仅通过态）
//!   ③ 种行是否存在（区分「真 404」与「原文丢失」两种报错文案）
//!   ④ tracker_urls 附加 tier（enabled + priority 排序）
//! announce 基址的读站点设定走共用的 `repo::settings`（此前这里是第
//! 三份逐字相同的局部 `setting()` 拷贝）。

use super::Repo;

/// ① 种名（None = 行不在或读失败，落盘名退回 "torrent"）。
pub async fn torrent_name(repo: &Repo, id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT name FROM torrents WHERE id = $1")
        .bind(id)
        .fetch_optional(&repo.db)
        .await
        .unwrap_or(None)
}

/// ② 原始 .torrent 字节；None = 种不在 / 非通过态 / 原文缺失（三合一，
/// 调用方靠 ③ 区分后两种）。
pub async fn raw_torrent_file(
    repo: &Repo,
    id: i64,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT f.raw FROM torrent_files f \
         JOIN torrents t ON t.id = f.torrent_id \
         WHERE f.torrent_id = $1 AND t.approval_status = 1",
    )
    .bind(id)
    .fetch_optional(&repo.db)
    .await
}

/// ③ 种行是否存在（读失败按「不存在」——宁可报 404 也不谎称原文丢失）。
pub async fn torrent_exists(repo: &Repo, id: i64) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1)")
        .bind(id)
        .fetch_one(&repo.db)
        .await
        .unwrap_or(false)
}

/// ④ 附加 tracker：`(url, with_passkey)`，enabled 且按 priority, id 排序。
/// 读失败给空列表（少几个 tier 不该让整个下载 500）。
pub async fn extra_trackers(repo: &Repo) -> Vec<(String, bool)> {
    sqlx::query_as(
        "SELECT url, with_passkey FROM tracker_urls WHERE enabled \
         ORDER BY priority, id",
    )
    .fetch_all(&repo.db)
    .await
    .unwrap_or_default()
}

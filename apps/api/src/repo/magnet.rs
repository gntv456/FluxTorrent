//! 磁力口的全部数据访问（`torrent_http/magnet.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」。
//!
//! 四个查询构成一条完整判定链，整体搬走才能让 H 层只剩编排：
//!   ① 取拼 magnet 的最小列（approval_status 兼作存在性检查）
//!   ② 非通过态时取发布者（暂缓/待审仅本人 + staff 可见）
//!   ③ 取下载者自己的 passkey
//!   ④ 付费种 + 是否已购（ES36 闸门要求磁力口与 .torrent 下载口对等）

use super::Repo;

/// ① `(info_hash, name, approval_status)`；None = 不可见或不存在。
pub async fn magnet_row(
    repo: &Repo,
    id: i64,
) -> Result<Option<(String, String, i16)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT info_hash, name, approval_status FROM torrents \
         WHERE id = $1 AND (approval_status = 1 OR approval_status = 0 \
            OR approval_status = 4)",
    )
    .bind(id)
    .fetch_optional(&repo.db)
    .await
}

/// ② 发布者 id（`fetch_one`：调用方已确认该行存在）。
pub async fn owner_id(repo: &Repo, id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(id)
        .fetch_one(&repo.db)
        .await
}

/// ③ 自己的 passkey（拼进 magnet 的 announce）。
pub async fn passkey(repo: &Repo, user_id: i64) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&repo.db)
        .await
}

/// ④ `(price, owner_id)`；None = 已被并发删除。
pub async fn price_owner(
    repo: &Repo,
    id: i64,
) -> Result<Option<(i64, Option<i64>)>, sqlx::Error> {
    sqlx::query_as("SELECT price, owner_id FROM torrents WHERE id = $1")
        .bind(id)
        .fetch_optional(&repo.db)
        .await
}

/// ④b 是否已购买。
pub async fn purchased(
    repo: &Repo,
    user_id: i64,
    torrent_id: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrent_purchases WHERE \
         user_id = $1 AND torrent_id = $2)",
    )
    .bind(user_id)
    .bind(torrent_id)
    .fetch_one(&repo.db)
    .await
}

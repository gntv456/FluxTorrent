//! 版块配置的版本戳读（`admin_http/forum_mods.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」——乐观并发的版本戳
//! 比对是 0376 新加的 H 层 SQL，属新增而非存量，按「新文件零容忍」处理。
//!
//! 为什么要 FOR UPDATE：并发窗口在「读戳」到「UPDATE」之间。若不加锁，
//! 两个带着同一版本戳的请求会先后通过校验、先后写成功，后写者照样
//! 覆盖前者——乐观并发就白做了。行锁把窗口收窄到本事务内。

use super::Repo;

/// 当前 `mtime`（None = 版块不存在）。
pub async fn mtime(
    repo: &Repo,
    forum_id: i64,
) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar("SELECT mtime FROM forums WHERE id = $1 FOR UPDATE")
        .bind(forum_id)
        .fetch_optional(&repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))
        .unwrap_or(None)
}

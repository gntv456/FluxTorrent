//! 信箱文件夹（pmboxes）的全部数据访问（`community_http/message_folders.rs`
//! 的 SQL 唯一居所）。2026-10-15 下沉：FRAME §2「H 不 import sqlx」。
//!
//! 整个 handler 原本就是一层薄壳（列表 / 新建 / 改名 / 删除），把 SQL
//! 整体搬到这里后 H 层只剩「鉴权 + 参数校验 + 错误码映射」。
//!
//! 两处口径（批一百三十四 P-322 定的，搬动时逐字保留）：
//!   · 改名/删除按 `id + user_id` 双条件命中，行数为 0 即「不是你的或
//!     不存在」→ NotFound，不静默回成功；
//!   · 新建前先数已有夹子（上限 3），数失败按 0 放行（不因一次读失败
//!     把人堵在门外）。

use super::Repo;
use sqlx::FromRow;

/// 文件夹行（含夹内未删信件数）。
#[derive(FromRow, serde::Serialize)]
pub struct PmBoxRow {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

/// 文件夹列表 + 夹内未删信件数。
pub async fn list_boxes(repo: &Repo, user_id: i64) -> Vec<PmBoxRow> {
    sqlx::query_as::<_, PmBoxRow>(
        "SELECT b.id, b.name, (SELECT count(*) FROM messages m WHERE \
         m.folder = b.id AND m.receiver_id = $1 AND m.location = 1) AS count \
         FROM pmboxes b WHERE b.user_id = $1 ORDER BY b.id",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| crate::errors::DomainError::Internal(e.into()))
    .unwrap_or_default()
}

/// 删除文件夹并连带清空夹内信。返回被删的夹子行数（0 = 未命中）。
pub async fn delete_box(repo: &Repo, user_id: i64, box_id: i64) -> i64 {
    sqlx::query("DELETE FROM messages WHERE folder = $2 AND \
                 receiver_id = $1")
        .bind(user_id)
        .bind(box_id)
        .execute(&repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))
        .unwrap_or_default();
    sqlx::query("DELETE FROM pmboxes WHERE id = $2 AND user_id = $1")
        .bind(user_id)
        .bind(box_id)
        .execute(&repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))
        .unwrap_or_default()
        .rows_affected() as i64
}

/// 已有文件夹数（上限判定用；读失败按 0 放行）。
pub async fn count_boxes(repo: &Repo, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pmboxes WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&repo.db)
        .await
        .unwrap_or(0)
}

/// 新建（同名则改名，UPSERT 返回 id）。
pub async fn upsert_box(
    repo: &Repo,
    user_id: i64,
    name: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO pmboxes (user_id, name) VALUES ($1, $2) \
         ON CONFLICT (user_id, name) DO UPDATE SET \
         name = EXCLUDED.name RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(&repo.db)
    .await
}

/// 改名。返回受影响行数（0 = 未命中）。
pub async fn rename_box(
    repo: &Repo,
    user_id: i64,
    box_id: i64,
    name: &str,
) -> i64 {
    sqlx::query(
        "UPDATE pmboxes SET name = $3 WHERE id = $2 AND user_id = $1",
    )
    .bind(user_id)
    .bind(box_id)
    .bind(name)
    .execute(&repo.db)
    .await
    .map_err(|e| crate::errors::DomainError::Internal(e.into()))
    .unwrap_or_default()
    .rows_affected() as i64
}

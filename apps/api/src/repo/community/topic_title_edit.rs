//! 改主题标题（PUT /forums/topics/{id}/title）的 SQL 唯一居所。
//!
//! 2026-11-10 随端点新建下沉（layer guard：新 H 层文件零 SQL）。
//! H 层只留判定（楼主/版主、长度、敏感词），这里管三件事：
//! 读主题归属、写新标题、给楼主落 PM 通知。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 主题的楼主与版块（None = 主题不存在）。
pub async fn topic_meta(
    repo: &Repo,
    tid: i64,
) -> DomainResult<Option<(Option<i64>, i64)>> {
    sqlx::query_as("SELECT user_id, forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

/// 改标题（不判定权限——调用方先过楼主/版主闸）。
pub async fn rename(repo: &Repo, tid: i64, title: &str) -> DomainResult<()> {
    sqlx::query("UPDATE topics SET title = $2 WHERE id = $1")
        .bind(tid)
        .bind(title)
        .execute(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 版主改他人标题时给楼主的 PM（与 post_edit 同口径；失败不影响主流程）。
pub async fn notify_owner(repo: &Repo, from: i64, to: i64) {
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(from)
    .bind(to)
    .bind("您的主题标题被编辑")
    .bind("您的主题标题已被管理组成员编辑，请查看最新内容。")
    .execute(&repo.db)
    .await;
}

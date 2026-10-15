//! 关注（/follows/*）的 SQL 唯一居所。
//!
//! 2026-10-16 下沉：FRAME §2「H 不 import sqlx」。community_http 域做
//! OpenAPI 注解化时按 300 行门禁拆出的新 H 文件（follow_read.rs 等）
//! 带着存量 SQL，按「新 H 层文件零容忍」处理——SQL 下沉到仓储层，
//! H 层只留鉴权、口径判断与 JSON 组装。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 关注行（用户/版块/主题三张表的 id + 展示名）。
pub type FollowRow = (i64, String);

/// 主题关注行额外带 forum_id（前端要据此跳版块）。
pub type FollowTopicRow = (i64, String, i64);

/// 我是否关注了该对象。
pub async fn is_following(
    repo: &Repo,
    user_id: i64,
    target_type: &str,
    target_id: i64,
) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM follows WHERE user_id = $1 AND \
         target_type = $2 AND target_id = $3)",
    )
    .bind(user_id)
    .bind(target_type)
    .bind(target_id)
    .fetch_one(&repo.db)
    .await
    .unwrap_or(false)
}

/// 该对象的关注人数。
pub async fn follower_count(
    repo: &Repo,
    target_type: &str,
    target_id: i64,
) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND \
         target_id = $2",
    )
    .bind(target_type)
    .bind(target_id)
    .fetch_one(&repo.db)
    .await
    .unwrap_or(0)
}

/// 我关注的用户（JOIN users 取 username）。
pub async fn my_user_follows(
    repo: &Repo,
    user_id: i64,
) -> DomainResult<Vec<FollowRow>> {
    Ok(sqlx::query_as(
        "SELECT u.id, u.username FROM follows f \
         JOIN users u ON u.id = f.target_id \
         WHERE f.user_id = $1 AND f.target_type = 'user' \
         ORDER BY f.created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 我关注的版块（JOIN forums 取 name）。
pub async fn my_forum_follows(
    repo: &Repo,
    user_id: i64,
) -> DomainResult<Vec<FollowRow>> {
    Ok(sqlx::query_as(
        "SELECT fc.id, fc.name FROM follows f \
         JOIN forums fc ON fc.id = f.target_id \
         WHERE f.user_id = $1 AND f.target_type = 'forum' \
         ORDER BY f.created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 我关注的主题（JOIN topics 取 title + forum_id）。
pub async fn my_topic_follows(
    repo: &Repo,
    user_id: i64,
) -> DomainResult<Vec<FollowTopicRow>> {
    Ok(sqlx::query_as(
        "SELECT t.id, t.title, t.forum_id FROM follows f \
         JOIN topics t ON t.id = f.target_id \
         WHERE f.user_id = $1 AND f.target_type = 'topic' \
         ORDER BY f.created_at DESC LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 清掉指向某主题的孤儿关注（target_id 无外键，删主题时应用层补）。
pub async fn drop_topic_follows(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<()> {
    sqlx::query(
        "DELETE FROM follows WHERE target_type = 'topic' \
         AND target_id = $1",
    )
    .bind(topic_id)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

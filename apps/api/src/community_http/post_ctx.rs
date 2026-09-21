//! M15 帖子上下文（post_context）。
//! 从 community_http.rs 按域拆出。

use crate::errors::{DomainError, DomainResult};

pub async fn post_context(
    db: &sqlx::PgPool,
    post_id: i64,
) -> DomainResult<Option<(i64, i64, i64)>> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT p.topic_id, t.forum_id, p.user_id FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         WHERE p.id = $1",
    )
    .bind(post_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row)
}

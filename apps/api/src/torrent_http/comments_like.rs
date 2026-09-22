//! 评论点赞 SQL 助手（0155 评论增强；从 interact.rs 拆出，300 行门禁）。

use crate::errors::{DomainError, DomainResult};

/// 单行 CTE 原子切换：已赞删行、未赞插入，返回「操作后处于已赞态」。
/// 两次往返的读-判-写在并发双击下会双双走插入（ON CONFLICT 吞掉），响应却都
/// 报 liked=true——CTE 一次往返天然消除。
pub(super) async fn toggle_like(
    db: &sqlx::PgPool,
    comment_id: i64,
    user_id: i64,
) -> DomainResult<bool> {
    let inserted = sqlx::query_scalar::<_, i64>(
        "WITH ins AS ( \
             INSERT INTO comment_likes (comment_id, user_id) \
             VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING 1) \
         SELECT count(*)::bigint FROM ins",
    )
    .bind(comment_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted > 0 {
        return Ok(true);
    }
    sqlx::query(
        "DELETE FROM comment_likes \
         WHERE comment_id = $1 AND user_id = $2",
    )
    .bind(comment_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(false)
}

//! 评论域（0155 点赞 + 0156 嵌套回复）：行类型 / 列表树序 / 发表归一 / 点赞切换。
//! 从 detail.rs 按域拆出（300 行门禁）。

use serde::Serialize;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::types::MAX_LIMIT;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub torrent_id: i64,
    pub username: Option<String>,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 点赞数（0155；viewer=0 无视角时恒 0）
    #[sqlx(default)]
    pub likes: i64,
    /// 当前用户是否已赞（0155）
    #[sqlx(default)]
    pub liked_by_me: bool,
    /// 嵌套回复（0156）：指向根评论 id；顶层评论为 NULL
    #[sqlx(default)]
    pub parent_id: Option<i64>,
    /// 被回复人用户名（显示层「回复 @xxx」；回复一楼时为 NULL）
    #[sqlx(default)]
    pub reply_to_user: Option<String>,
}

/// viewer 版评论列表：likes 全员一致；liked_by_me 是 viewer 态。
/// 0156 树序：根楼层倒序、组内回复正序（时间线可读，回复不打乱楼层）。
pub async fn list_comments_as(
    db: &PgPool,
    torrent_id: i64,
    limit: i64,
    viewer: i64,
) -> DomainResult<Vec<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.torrent_id, u.username, c.body, c.created_at, \
         (SELECT count(*) FROM comment_likes cl \
            WHERE cl.comment_id = c.id) AS likes, \
         EXISTS(SELECT 1 FROM comment_likes cl \
            WHERE cl.comment_id = c.id AND cl.user_id = $3) AS liked_by_me, \
         c.parent_id, c.reply_to_user \
         FROM comments c LEFT JOIN users u ON u.id = c.user_id \
         WHERE c.torrent_id = $1 \
         ORDER BY COALESCE(c.parent_id, c.id) DESC, c.id ASC LIMIT $2",
    )
    .bind(torrent_id)
    .bind(limit.clamp(1, MAX_LIMIT))
    .bind(viewer)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 0156 嵌套回复版发表：parent_id 是「回复目标评论 id」——
/// 存储层把它归一到根（回复某条回复 → 挂到该回复的根楼层，组内按时间正序）；
/// reply_to_user 取被回复人用户名用于「回复 @xxx」显示。
pub async fn add_comment_as(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
    body: &str,
    parent_id: Option<i64>,
) -> DomainResult<i64> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND \
         approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(torrent_id));
    }
    if body.trim().is_empty() {
        return Err(DomainError::Validation("评论不能为空".into()));
    }
    // 归一：目标必须存在且属于同一种子；回复的回复挂到根
    let (root_id, reply_to_user): (Option<i64>, Option<String>) =
        match parent_id {
            Some(pid) => {
                let target: Option<(Option<i64>, i64)> = sqlx::query_as(
                    "SELECT parent_id, user_id FROM comments \
                     WHERE id = $1 AND torrent_id = $2",
                )
                .bind(pid)
                .bind(torrent_id)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                let Some((parent_of_target, target_user)) = target else {
                    return Err(DomainError::Validation(
                        "被回复的评论不存在".into(),
                    ));
                };
                let root = parent_of_target.unwrap_or(pid);
                let uname: Option<String> = sqlx::query_scalar(
                    "SELECT username FROM users WHERE id = $1",
                )
                .bind(target_user)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
                (Some(root), uname.filter(|_| parent_of_target.is_some()))
            }
            None => (None, None),
        };
    // 禁言同样要挡住种子评论（0285）：旧版只有论坛与漂流瓶读 forumpost，
    // 被禁言的人换个通道照样发言。
    if crate::community_http::is_muted(db, user_id).await {
        return Err(DomainError::Validation("账号已被禁言，暂不能评论".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO comments (torrent_id, user_id, body, parent_id, \
         reply_to_user) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(torrent_id)
    .bind(user_id)
    .bind(body)
    .bind(root_id)
    .bind(reply_to_user)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

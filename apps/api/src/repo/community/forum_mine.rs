//! 「我参与的论坛活动」查询（H 层 community_http::forums_mine 的唯一 SQL 居所）。
//!
//! 2026-10-17 随「论坛补齐样图」新增：样图 § forum-feed 的三个标签里，
//! 「我参与的」这一档本站此前**没有任何出口**——`/users/{id}` 的
//! `recent_posts` 写死 5 条且只有回复，既不能翻页也看不出总数。
//!
//! ⚠️ 两块都必须带可读性谓词 `f.minclassread <= 读者等级`（口径与
//! `forum_side.rs` 一致）：用户被降级后，不该在「我的主题」里继续看到
//! 已无读权版块的标题。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 我发的主题（含我自己在里面的回复数，便于界面说「我回了 N 条」）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MyTopic {
    pub id: i64,
    pub title: String,
    pub forum_id: i64,
    pub forum_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_post_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 总回复数（楼主帖不计；零帖主题取 0，不取 -1）
    pub replies: i64,
    /// 其中我自己回的条数
    pub my_replies: i64,
}

/// 我发的回复（楼主帖不算回复：与 `replies` 同一口径，
/// 判据是「该主题里 id 最小的那帖」）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MyReply {
    pub id: i64,
    pub topic_id: i64,
    pub topic_title: String,
    pub forum_id: i64,
    pub forum_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 正文摘要（前台不渲染整帖，80 字够认是哪一条）
    pub excerpt: String,
}

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

const MY_TOPICS_WHERE: &str = "t.user_id = $1 AND f.minclassread <= $2";

const MY_REPLIES_WHERE: &str = "p.user_id = $1 \
    AND f.minclassread <= $2 \
    AND p.id > (SELECT min(p0.id) FROM posts p0 \
                 WHERE p0.topic_id = p.topic_id)";

pub async fn my_topics(
    repo: &Repo,
    uid: i64,
    class_id: i32,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<MyTopic>> {
    sqlx::query_as::<_, MyTopic>(&format!(
        "SELECT t.id, t.title, t.forum_id, f.name AS forum_name, \
            t.created_at, \
            t.last_post_at, \
            greatest((SELECT count(*) FROM posts p \
                       WHERE p.topic_id = t.id) - 1, 0) AS replies, \
            (SELECT count(*) FROM posts p2 WHERE p2.topic_id = t.id \
               AND p2.user_id = $1 \
               AND p2.id > (SELECT min(p3.id) FROM posts p3 \
                             WHERE p3.topic_id = t.id)) AS my_replies \
         FROM topics t JOIN forums f ON f.id = t.forum_id \
         WHERE {MY_TOPICS_WHERE} \
         ORDER BY COALESCE(t.last_post_at, t.created_at) DESC, t.id DESC \
         LIMIT $3 OFFSET $4"
    ))
    .bind(uid)
    .bind(class_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&repo.db)
    .await
    .map_err(internal)
}

pub async fn count_my_topics(
    repo: &Repo,
    uid: i64,
    class_id: i32,
) -> DomainResult<i64> {
    sqlx::query_scalar(&format!(
        "SELECT count(*) FROM topics t JOIN forums f ON f.id = t.forum_id \
         WHERE {MY_TOPICS_WHERE}"
    ))
    .bind(uid)
    .bind(class_id)
    .fetch_one(&repo.db)
    .await
    .map_err(internal)
}

pub async fn my_replies(
    repo: &Repo,
    uid: i64,
    class_id: i32,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<MyReply>> {
    sqlx::query_as::<_, MyReply>(&format!(
        "SELECT p.id, p.topic_id, t.title AS topic_title, \
            t.forum_id, f.name AS forum_name, p.created_at, \
            left(COALESCE(NULLIF(p.body_text, ''), p.body), 80) AS excerpt \
         FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         JOIN forums f ON f.id = t.forum_id \
         WHERE {MY_REPLIES_WHERE} \
         ORDER BY p.id DESC LIMIT $3 OFFSET $4"
    ))
    .bind(uid)
    .bind(class_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&repo.db)
    .await
    .map_err(internal)
}

pub async fn count_my_replies(
    repo: &Repo,
    uid: i64,
    class_id: i32,
) -> DomainResult<i64> {
    sqlx::query_scalar(&format!(
        "SELECT count(*) FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         JOIN forums f ON f.id = t.forum_id \
         WHERE {MY_REPLIES_WHERE}"
    ))
    .bind(uid)
    .bind(class_id)
    .fetch_one(&repo.db)
    .await
    .map_err(internal)
}

/// 最近读过的主题（个人中心「最近阅读主题」表体）。
///
/// `topic_reads` 主键是 `(user_id, topic_id)`，一人一主题只有一行，
/// 所以「最近阅读」天然去重，按 `read_at` 倒序即是。
/// ⚠️ 同样必须带 `f.minclassread <= 读者等级`：读者被降级后，
/// 他的阅读史里可能留着已无读权版块的标题。
///
/// 元组顺序即 JSON 顺序：topic_id / title / forum_id / forum_name /
/// replies / read_at。
#[allow(clippy::type_complexity)]
pub async fn recent_read_topics(
    repo: &Repo,
    uid: i64,
    class_id: i32,
    limit: i64,
) -> DomainResult<
    Vec<(i64, String, i64, String, i64, chrono::DateTime<chrono::Utc>)>,
> {
    sqlx::query_as(
        "SELECT t.id, t.title, t.forum_id, f.name, \
            greatest((SELECT count(*) FROM posts p \
                       WHERE p.topic_id = t.id) - 1, 0) AS replies, \
            r.read_at \
         FROM topic_reads r \
         JOIN topics t ON t.id = r.topic_id \
         JOIN forums f ON f.id = t.forum_id \
         WHERE r.user_id = $1 AND f.minclassread <= $2 \
         ORDER BY r.read_at DESC LIMIT $3",
    )
    .bind(uid)
    .bind(class_id)
    .bind(limit)
    .fetch_all(&repo.db)
    .await
    .map_err(internal)
}

//! 论坛首页右栏与版块列表计数的 SQL 唯一居所。
//!
//! 2026-10-17 随「论坛对齐样图」新增：H 层（community_http）零容忍带
//! SQL，故统计/热帖/最新回复与主题总数计数都落在这里。
//!
//! ⚠️ 三块都是**跨版块**聚合，每一段都必须带可读性谓词
//! `f.minclassread <= 读者等级`——否则热帖榜/最新回复会把隐藏版块的
//! 标题泄漏给无读权用户（口径与 forum_list / list_boards 一致）。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 今日热帖的统计窗口（天）。按窗口内回复数排序，不是历史总回复——
/// 后者会让老帖永远霸榜，失去「近期在聊什么」的意义。
///
/// ⚠️ 必须是 i32：`make_interval(days => $n)` 只接受 int，绑 i64 会在
/// 运行期报 `function make_interval(days => bigint) does not exist`
/// （cargo check 查不出，13:40 实测踩到）。
pub const HOT_WINDOW_DAYS: i32 = 7;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct HotTopic {
    pub id: i64,
    pub title: String,
    pub forum_name: String,
    pub replies: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct LatestReply {
    pub id: i64,
    pub topic_id: i64,
    pub topic_title: String,
    pub forum_name: String,
    pub username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 论坛统计：可读主题数 / 可读回帖数 / 今日新主题数。
pub async fn side_stats(
    repo: &Repo,
    class_id: i32,
) -> DomainResult<(i64, i64, i64)> {
    Ok(sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM topics t JOIN forums f ON f.id = t.forum_id \
              WHERE f.minclassread <= $1), \
            (SELECT count(*) FROM posts p JOIN topics t ON t.id = p.topic_id \
              JOIN forums f ON f.id = t.forum_id WHERE f.minclassread <= $1), \
            (SELECT count(*) FROM topics t JOIN forums f ON f.id = t.forum_id \
              WHERE f.minclassread <= $1 \
                AND t.created_at >= date_trunc('day', now()))",
    )
    .bind(class_id)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 今日热帖 top5（窗口内按回复数降序）。
///
/// ⚠️ 回复数 = 总帖数 - 楼主帖，但**零帖主题真实存在**（现网 11 个，
/// 多为探针/迁移遗留），不减出负数要靠 greatest——现网
/// `/forums/{id}/topics` 就在回 `"replies":-1`，同一口径两处一起修。
pub async fn hot_topics(
    repo: &Repo,
    class_id: i32,
) -> DomainResult<Vec<HotTopic>> {
    Ok(sqlx::query_as(
        "SELECT t.id, t.title, f.name AS forum_name, \
            greatest( \
              (SELECT count(*) FROM posts p WHERE p.topic_id = t.id) - 1, 0 \
            ) AS replies \
         FROM topics t JOIN forums f ON f.id = t.forum_id \
         WHERE f.minclassread <= $1 \
           AND COALESCE(t.last_post_at, t.created_at) \
               >= now() - make_interval(days => $2) \
         ORDER BY replies DESC, t.id DESC LIMIT 5",
    )
    .bind(class_id)
    .bind(HOT_WINDOW_DAYS)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 最新回复 top5：排除楼主帖本身（`p.id` 大于该主题的最小帖 id 即「有回复」）。
pub async fn latest_replies(
    repo: &Repo,
    class_id: i32,
) -> DomainResult<Vec<LatestReply>> {
    Ok(sqlx::query_as(
        "SELECT p.id, p.topic_id, t.title AS topic_title, \
            f.name AS forum_name, u.username, p.created_at \
         FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         JOIN forums f ON f.id = t.forum_id \
         LEFT JOIN users u ON u.id = p.user_id \
         WHERE f.minclassread <= $1 \
           AND p.id > (SELECT min(p2.id) FROM posts p2 \
                        WHERE p2.topic_id = p.topic_id) \
         ORDER BY p.id DESC LIMIT 5",
    )
    .bind(class_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 某版块符合筛选条件的主题总数（分页用）。
///
/// ⚠️ 必须与 `topic_list` 的 WHERE **完全同式**（含 tag 谓词），否则
/// 前端按 total 画出的页码里会有一批空页。`where_sql` 由调用方把
/// 已拼好 tag 谓词的片段传进来——谓词里的 tag id 已在 H 层解析成
/// i32 后才格式化，无注入面。
pub async fn count_board_topics(
    repo: &Repo,
    fid: i64,
    where_sql: &str,
) -> DomainResult<i64> {
    let sql = format!("SELECT count(*) FROM topics t WHERE {where_sql}");
    sqlx::query_scalar::<_, i64>(&sql)
        .bind(fid)
        .fetch_one(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

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

/// 论坛首页中栏话题流（样图集 § tplForum 中列）：全站可读版块的最新
/// 主题窗口（置顶优先，然后按最后活动时间倒序，无翻页）。
///
/// 可见性谓词与 `forum_list` 逐字同口径（minclassread / 版主兼任 /
/// 隐藏分区对非 staff 不可见）——两处必须一致，否则首页会列出左栏
/// 根本没有的版块。`only` 白名单在 H 层已校验（sticky|digest），
/// 这里只做常量映射，不把原始输入拼进 SQL。
///
/// 行上比 topic_list 多两列（样图要的）：版块名与最后回复者。
/// 回复数口径与 `hot_topics` 一致：总帖数 - 楼主帖，下限 0。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct RecentTopic {
    pub id: i64,
    pub forum_id: i64,
    pub forum_name: String,
    pub title: String,
    pub username: Option<String>,
    pub replies: i64,
    pub views: i32,
    pub last_post_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sticky: bool,
    pub locked: bool,
    pub digest: bool,
    pub topic_type: String,
    /// 标签（0123）：json 数组，随行回（与 topic_list 同口径）
    pub tags: serde_json::Value,
    /// 最后回复者；零回帖主题为 NULL（前端回落主题作者）
    pub last_replier: Option<String>,
}

/// 首页话题流行数（样图一屏约 13 行，取 30 覆盖两屏；更多历史去
/// 版块页 / 关注流，这里不做翻页）。
pub const RECENT_LIMIT: i64 = 30;

pub async fn recent_topics(
    repo: &Repo,
    class_id: i32,
    user_id: i64,
    is_staff: bool,
    only: Option<&str>,
) -> DomainResult<Vec<RecentTopic>> {
    // 白名单常量映射（H 层已校验，这里双保险）
    let type_filter = match only {
        Some("sticky") => " AND t.sticky",
        Some("digest") => " AND t.digest",
        _ => "",
    };
    let sql = format!(
        "SELECT t.id, t.forum_id, f.name AS forum_name, t.title, \
            u.username, \
            greatest( \
              (SELECT count(*) FROM posts p WHERE p.topic_id = t.id) - 1, 0 \
            ) AS replies, t.views, t.last_post_at, \
            t.sticky, t.locked, t.digest, t.topic_type, \
            COALESCE( \
              (SELECT json_agg(json_build_object('id', d.id, \
                     'name', d.name, 'kind', d.kind, \
                     'bg_color', d.bg_color, 'color', d.color, \
                     'font_size', d.font_size, 'margin', d.margin, \
                     'padding', d.padding, \
                     'border_radius', d.border_radius)) \
                 FROM (SELECT td.* FROM tag_dict td \
                         JOIN topic_tags tt2 ON tt2.tag_id = td.id \
                        WHERE tt2.topic_id = t.id AND td.scope = 'forum' \
                        ORDER BY td.sort, td.id) d), \
              '[]'::json) AS tags, \
            (SELECT u2.username FROM posts p2 \
               JOIN users u2 ON u2.id = p2.user_id \
              WHERE p2.topic_id = t.id ORDER BY p2.id DESC LIMIT 1) \
              AS last_replier \
         FROM topics t \
         JOIN forums f ON f.id = t.forum_id \
         LEFT JOIN forum_categories c ON c.id = f.category_id \
         LEFT JOIN users u ON u.id = t.user_id \
         WHERE (f.minclassread <= $1 OR EXISTS ( \
                 SELECT 1 FROM forum_mods fm \
                  WHERE fm.forum_id = f.id AND fm.user_id = $2)) \
           AND (c.id IS NULL OR c.visible = TRUE OR $3) \
           {type_filter} \
         ORDER BY t.sticky DESC, \
                  COALESCE(t.last_post_at, t.created_at) DESC, t.id DESC \
         LIMIT {RECENT_LIMIT}"
    );
    Ok(sqlx::query_as(&sql)
        .bind(class_id)
        .bind(user_id)
        .bind(is_staff)
        .fetch_all(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?)
}

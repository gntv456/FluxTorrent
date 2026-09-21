//! M15 关注流（forum_feed）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct FeedRow {
    topic_id: i64,
    title: String,
    forum_id: i64,
    forum_name: Option<String>,
    username: Option<String>,
    #[sqlx(default)]
    topic_type: String,
    last_post_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    sticky: bool,
    #[sqlx(default)]
    locked: bool,
    replies: i64,
    /// 命中的关注来源：forum=关注版块 / user=关注作者 / topic=关注主题
    via: String,
}

#[derive(Deserialize)]
struct FeedQuery {
    #[serde(default)]
    limit: Option<i64>,
    /// 游标分页（0123）：只取 activity < before 的行；activity = COALESCE(last_post_at, created_at)
    #[serde(default)]
    before: Option<chrono::DateTime<chrono::Utc>>,
}

/// 关注流：我关注的版块/用户的新主题 + 我关注主题的最新回复，按最后活动时间倒序。
/// 权限：逐行复刻 forum_access 的 can_read（`class_id>=90` 放行 / 版主放行 / `class_id>=minclassread`），
/// 否则用户降级后会在流里看到已无权限版块的标题（越权泄露）。
#[get("/forums/feed")]
async fn forum_feed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FeedQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    // 多取一些（UNION 可能同一主题命中多路），去重后再截到 limit
    let fetch = (limit * 2).min(200);
    let rows: Vec<FeedRow> = sqlx::query_as(
        "SELECT * FROM ( \
           SELECT t.id AS topic_id, t.title, t.forum_id, fc.name AS forum_name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) AS replies, \
                  'forum' AS via \
             FROM follows fl \
             JOIN topics t ON t.forum_id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'forum' \
           UNION ALL \
           SELECT t.id, t.title, t.forum_id, fc.name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id), \
                  'user' \
             FROM follows fl \
             JOIN topics t ON t.user_id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'user' \
           UNION ALL \
           SELECT t.id, t.title, t.forum_id, fc.name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id), \
                  'topic' \
             FROM follows fl \
             JOIN topics t ON t.id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'topic' \
         ) x \
         WHERE ($4::timestamptz IS NULL OR COALESCE(x.last_post_at, x.created_at) < $4) \
           AND ($2 >= 90 OR x.forum_id IN ( \
                 SELECT fc2.id FROM forums fc2 WHERE fc2.minclassread <= $2 \
             ) OR EXISTS(SELECT 1 FROM forum_mods fm WHERE fm.forum_id = x.forum_id AND fm.user_id = $1)) \
         ORDER BY x.last_post_at DESC NULLS LAST, x.topic_id DESC LIMIT $3",
    )
    .bind(auth.id)
    .bind(auth.class_id)
    .bind(fetch)
    .bind(q.before)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 同一主题可能同时因「关注版块」和「关注作者」命中，按 topic_id 去重（保留首次=最后活动最新的那条）
    let mut seen = std::collections::HashSet::new();
    let items: Vec<&FeedRow> = rows
        .iter()
        .filter(|r| seen.insert(r.topic_id))
        .take(limit as usize)
        .collect();
    // 下一页游标：末行的最后活动时间（取到 limit 整页才可能有下一页；与主题页 before=post_id 同范式）
    let next_before = if items.len() as i64 == limit {
        items
            .last()
            .and_then(|r| r.last_post_at.or(Some(r.created_at)))
    } else {
        None
    };
    Ok(ok(
        serde_json::json!({ "items": items, "next_before": next_before }),
    ))
}

// ---- 论坛悬赏（0124）：发帖冻结（topic_create 内 spend_spark_tx）→ 楼主采纳发放 ----

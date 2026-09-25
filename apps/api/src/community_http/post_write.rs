//! M15 帖子写入（回复/编辑/删除/点赞/收藏）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct ReplyReq {
    body: String,
    /// 楼中楼（0163）：回复的目标楼层 id；空 = 回主题（顶层楼）
    reply_to: Option<i64>,
}

#[post("/forums/topics/{id}/reply")]
async fn post_reply(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    if body.body.trim().is_empty() {
        return Err(DomainError::Validation("回复不能为空".into()));
    }
    // 敏感词（Phase3）
    check_banned_words(&state.repo.db, &body.body).await?;
    // 视频内嵌（0190）：每帖视频块上限
    check_video_count(&state.repo.db, &body.body).await?;
    let row: Option<(i64, bool, i64, String)> = sqlx::query_as(
        "SELECT forum_id, locked, COALESCE(user_id, 0), \
         title FROM topics WHERE id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((fid, locked, author, title)) = row else {
        return Err(DomainError::NotFound(tid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_write {
        return Err(DomainError::Forbidden);
    }
    if locked && !perm.can_mod {
        return Err(DomainError::Validation("主题已锁定".into()));
    }
    forum_flood_check(&state.repo.db, auth.id, auth.class_id).await?;
    let body_text = strip_markdown(&body.body);
    // 楼中楼（0163）：解析回复目标楼 → root_id/parent_id（目标楼须属于本主题；
    // 目标是顶层楼则 root = 目标楼 id，目标是楼中楼则 root = 它的 root_id）
    let (root_id, parent_id, parent_author) = match body.reply_to {
        Some(pt) => {
            let row: Option<(i64, Option<i64>, Option<i64>)> = sqlx::query_as(
                "SELECT COALESCE(root_id, id), root_id, user_id FROM posts \
                 WHERE id = $1 AND topic_id = $2",
            )
            .bind(pt)
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            match row {
                Some((r, _, pa)) => (Some(r), Some(pt), pa),
                None => (None, None, None),
            }
        }
        None => (None, None, None),
    };
    let post_id: i64 = sqlx::query_scalar(
        "INSERT INTO posts (id, topic_id, user_id, body, body_text, \
         root_id, parent_id) \
         VALUES (nextval('posts_id_seq'), $1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tid)
    .bind(auth.id)
    .bind(&body.body)
    .bind(&body_text)
    .bind(root_id)
    .bind(parent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE topics SET last_post_at = now() WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    forum_flood_mark(&state.repo.db, auth.id).await;
    // 回帖 +1 火花
    let idem = format!("forum-reply:{}:{}", auth.id, post_id);
    earn_spark(&state.repo.db, auth.id, 1, "forum", &idem).await?;
    // 通知楼主有新回复（自己回自己不通知）；@提及 排除楼主，避免同一件事两条通知
    if author != auth.id && author != 0 {
        notify_user(
            &state.repo.db,
            author,
            "主题收到回复",
            &format!(
                "用户 #{} 回复了你的主题：[{}](/forums/topic/{})",
                auth.id, title, tid
            ),
        )
        .await;
    }
    // 楼中楼（0163）：被回复楼层作者收到定向通知（自己/匿名楼不通知）
    if let Some(pa) = parent_author {
        if pa != auth.id {
            notify_user(
                &state.repo.db,
                pa,
                "你的楼层收到回复",
                &format!(
                    "用户 #{} 回复了你在[{}](/forums/topic/{})的楼层",
                    auth.id, title, tid
                ),
            )
            .await;
        }
    }
    notify_mentions(
        &state.repo.db,
        &body.body,
        auth.id,
        tid,
        &title,
        "回复中",
        &[author],
    )
    .await;
    // 关注了本主题的人：有新回复时通知（楼主已由上面「主题收到回复」通知过，回帖者自己更不必通知）
    notify_followers(
        &state.repo.db,
        "topic",
        tid,
        "关注的主题有新回复",
        &format!("[{}](/forums/topic/{}) · 来自用户 #{}", title, tid, auth.id),
        &[author, auth.id],
    )
    .await;
    Ok(ok(serde_json::json!({ "post_id": post_id })))
}

// ---- 论坛关注订阅（0121）：用户 / 版块 / 主题 ----
//
// 语义分工（刻意的）：
//   · 关注**用户** → 他发新主题时给关注者发一条通知（关注某人是明确意图，通知不算打扰）；
//   · 关注**版块** → 只进「关注流」，不发通知（版块流量大，逐个通知必成刷屏）；
//   · 关注**主题** → 有人回帖时给关注者发通知（等价于「订阅该主题」，这才是关注主题的意义）。
// 通知复用既有 `messages`（sender_id IS NULL = 系统通知），不新建通知表。

//! 改主题标题（topic_title_edit）。
//!
//! 标题存在 topics.title，而 `PUT /forums/posts/{id}` 只改 posts.body：
//! 楼主连自己首帖的标题都改不了，全站也没有任何写 topics.title 的口子
//! （版主的 `/manage` 只有置顶/锁定/精华/公告/移动）。此端点补上这一段，
//! 口径对齐 post_edit：作者本人或本版版主可改，无时限，改后留审计。

use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 与发帖页 `forum-composer.tsx` 的 `maxLength={120}` 同口径。
const TITLE_MAX: usize = 120;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct TopicTitleEditReq {
    title: String,
}

#[utoipa::path(
    put,
    path = "/api/v1/forums/topics/{id}/title",
    tag = "社区/论坛",
    description = "改主题标题（楼主或版主；过敏感词闸，留审计）。",
    params(
        ("id" = i64, description = "主题 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非楼主且无编辑权（code 2003）"),
        (status = 404, description = "主题不存在（code 2004）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[put("/forums/topics/{id}/title")]
pub(crate) async fn topic_title_edit(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicTitleEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let title = body.title.trim();
    if title.is_empty() {
        return Err(DomainError::Validation("标题不能为空".into()));
    }
    if title.chars().count() > TITLE_MAX {
        // 静态字面量：validation_i18n 门禁禁 format! 拼句（无法按原句查译文表）
        return Err(DomainError::Validation("标题最多 120 个字符".into()));
    }
    // 发帖时标题与正文一起过闸；改标题是同一条绕过路径，必须同样过。
    check_banned_words(&state.repo.db, title).await?;
    let tid = path.into_inner();
    let meta: Option<(Option<i64>, i64)> =
        sqlx::query_as("SELECT user_id, forum_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((op_id, fid)) = meta else {
        return Err(DomainError::NotFound(tid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if op_id != Some(auth.id) && !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE topics SET title = $2 WHERE id = $1")
        .bind(tid)
        .bind(title)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 版主改他人标题：自动 PM 通知楼主（与 post_edit 同口径）
    if let Some(uid) = op_id.filter(|u| *u != auth.id) {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES ($1, $2, $3, $4)",
        )
        .bind(auth.id)
        .bind(uid)
        .bind("您的主题标题被编辑")
        .bind("您的主题标题已被管理组成员编辑，请查看最新内容。")
        .execute(&state.repo.db)
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.topic.rename", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "edited": tid, "title": title })))
}

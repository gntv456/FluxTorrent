//! M15 论坛投票（vote/close）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct PollVoteReq {
    topic_id: i64,
    option_index: i32,
}

/// 投一票：登录即可投（版块 can_read 再验一次 forum_access）。
/// 免费（趣味盒扣 1 魔力是游戏口径，论坛投票是表达渠道）。校验全部前置再落占位（对齐 fun_vote 的竞态修复）。
#[post("/forums/poll/vote")]
async fn poll_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PollVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: Option<(serde_json::Value, bool, i64)> = sqlx::query_as(
        "SELECT tp.options, tp.closed, t.forum_id FROM topic_polls tp \
         JOIN topics t ON t.id = tp.topic_id WHERE tp.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((options, closed, fid)) = p else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if closed {
        return Err(DomainError::Validation("投票已截止".into()));
    }
    let n = options.as_array().map(|a| a.len()).unwrap_or(0);
    if body.option_index < 0 || body.option_index as usize >= n {
        return Err(DomainError::Validation("选项无效".into()));
    }
    let voted = sqlx::query(
        "INSERT INTO poll_votes (topic_id, user_id, option_index) VALUES ($1, $2, $3) \
         ON CONFLICT (topic_id, user_id) DO NOTHING",
    )
    .bind(body.topic_id)
    .bind(auth.id)
    .bind(body.option_index)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if voted == 0 {
        return Err(DomainError::Validation("已经投过啦，一人一票".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.poll_vote", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "option_index": body.option_index }),
    ))
}

#[derive(Deserialize)]
struct PollCloseReq {
    topic_id: i64,
}

/// 楼主提前截止投票（截止后只读结果；与 fun_polls.closed 同语义）。版主亦可截止（治理口径）。
#[post("/forums/poll/close")]
async fn poll_close(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PollCloseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: Option<(i64, i64)> = sqlx::query_as(
        "SELECT t.user_id, t.forum_id FROM topic_polls tp JOIN topics t ON t.id = tp.topic_id \
         WHERE tp.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((op, fid)) = p else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if auth.id != op && !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("UPDATE topic_polls SET closed = TRUE WHERE topic_id = $1 AND NOT closed")
        .bind(body.topic_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("投票已截止".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.poll_close", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "closed": true }),
    ))
}

// ---- 论坛抽奖（0126）：发帖冻结奖金池（topic_create）→ 参与 → 开奖 ----

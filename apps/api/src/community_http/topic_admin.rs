//! M15 主题管理（置顶/锁定/移动）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 主题管理：置顶/锁定/移动版块（版主限本版块）
#[derive(Deserialize)]
struct TopicManageReq {
    #[serde(default)]
    sticky: Option<bool>,
    #[serde(default)]
    locked: Option<bool>,
    /// 精华帖（NP digest 口径）：版主标记，列表/详情加精徽标
    #[serde(default)]
    digest: Option<bool>,
    #[serde(default)]
    move_to_forum_id: Option<i64>,
}

#[post("/forums/topics/{id}/manage")]
async fn topic_manage(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicManageReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let fid: Option<i64> =
        sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let mut moved = false;
    if let Some(target) = body.move_to_forum_id {
        if target != fid {
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM forums WHERE id = $1")
                    .bind(target)
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            if exists.is_none() {
                return Err(DomainError::Validation("目标版块不存在".into()));
            }
            sqlx::query("UPDATE topics SET forum_id = $1 WHERE id = $2")
                .bind(target)
                .bind(tid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            moved = true;
        }
    }
    if let Some(sticky) = body.sticky {
        sqlx::query("UPDATE topics SET sticky = $1 WHERE id = $2")
            .bind(sticky)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(locked) = body.locked {
        sqlx::query("UPDATE topics SET locked = $1 WHERE id = $2")
            .bind(locked)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(digest) = body.digest {
        sqlx::query("UPDATE topics SET digest = $1 WHERE id = $2")
            .bind(digest)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.topic_manage", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "managed": tid, "moved": moved })))
}

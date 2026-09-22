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

/// 批量主题管理（版块页「版主模式」工具栏）。
///
/// 与单主题口径一致：**逐个校验涉及版块的 `can_mod`，有一个无权就整体拒绝**——
/// 不做「部分成功」，否则版主无法从响应判断哪些生效了、哪些没有。
/// 单条 UPDATE 用 COALESCE 合并各行改动，避免 N 次往返。
#[derive(Deserialize)]
struct TopicManageBatchReq {
    ids: Vec<i64>,
    #[serde(default)]
    sticky: Option<bool>,
    #[serde(default)]
    locked: Option<bool>,
    #[serde(default)]
    digest: Option<bool>,
    #[serde(default)]
    move_to_forum_id: Option<i64>,
}

/// 一次最多处理多少条（防误全选整站 + 防超长 SQL 参数）
const BATCH_MAX: usize = 200;

#[post("/forums/topics/manage-batch")]
async fn topic_manage_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicManageBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("请先选择要操作的主题".into()));
    }
    if body.ids.len() > BATCH_MAX {
        return Err(DomainError::Validation(format!(
            "一次最多处理 {BATCH_MAX} 个主题"
        )));
    }
    if body.sticky.is_none()
        && body.locked.is_none()
        && body.digest.is_none()
        && body.move_to_forum_id.is_none()
    {
        return Err(DomainError::Validation("没有要执行的操作".into()));
    }
    // 主题 → 所属版块（不存在的 id 直接忽略；一个都不存在则 404）
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, forum_id FROM topics WHERE id = ANY($1::bigint[])",
    )
    .bind(&body.ids)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if rows.is_empty() {
        return Err(DomainError::NotFound(0));
    }
    let mut fids: Vec<i64> = rows.iter().map(|r| r.1).collect();
    fids.sort_unstable();
    fids.dedup();
    for fid in &fids {
        let perm =
            forum_access(&state.repo.db, auth.id, auth.class_id, *fid).await?;
        if !perm.can_mod {
            return Err(DomainError::Forbidden);
        }
    }
    if let Some(target) = body.move_to_forum_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM forums WHERE id = $1)",
        )
        .bind(target)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !exists {
            return Err(DomainError::Validation("目标版块不存在".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE topics SET \
            sticky = COALESCE($2::boolean, sticky), \
            locked = COALESCE($3::boolean, locked), \
            digest = COALESCE($4::boolean, digest), \
            forum_id = COALESCE($5::bigint, forum_id) \
         WHERE id = ANY($1::bigint[])",
    )
    .bind(&body.ids)
    .bind(body.sticky)
    .bind(body.locked)
    .bind(body.digest)
    .bind(body.move_to_forum_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "forum.topic_manage_batch", None)
        .await;
    Ok(ok(serde_json::json!({ "updated": n })))
}

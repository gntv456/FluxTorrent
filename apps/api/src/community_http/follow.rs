//! M15 关注订阅（用户/版块/主题 + 关注流）。
//! 从 community_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub(crate) fn normalize_follow_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "user" => Some("user"),
        "forum" => Some("forum"),
        "topic" => Some("topic"),
        _ => None,
    }
}

/// P-314 同族：`target_type` 拼成 `type`、`target_id` 拼成 `id` 都会因缺字段
/// 报错，但**多带一个键**旧写法静默接受并回 200——例如客户端从别处抄来的
/// `following: true`，接口照样报「关注成功」，实际状态由服务端算。
/// 前端载荷只有这两个键（forum-follow.tsx:82-85），加这条零行为变化。
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct FollowReq {
    target_type: String,
    target_id: i64,
}

/// 校验关注目标存在 + 当前用户对其有可见权限；返回规范化后的 type。
/// 自关注被拒（否则自己的新主题会通知自己）。
async fn validate_follow_target(
    db: &sqlx::PgPool,
    auth: &crate::http::AuthUser,
    ttype: &str,
    tid: i64,
) -> DomainResult<()> {
    match ttype {
        "user" => {
            if tid == auth.id {
                return Err(DomainError::Validation("不能关注自己".into()));
            }
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id = \
                 $1 AND status < 2)",
            )
            .bind(tid)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::NotFound(tid));
            }
        }
        "forum" => {
            let perm = forum_access(db, auth.id, auth.class_id, tid).await?;
            if !perm.can_read {
                return Err(DomainError::NotFound(tid));
            }
        }
        "topic" => {
            let fid: Option<i64> =
                sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
                    .bind(tid)
                    .fetch_optional(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(fid) = fid else {
                return Err(DomainError::NotFound(tid));
            };
            let perm = forum_access(db, auth.id, auth.class_id, fid).await?;
            if !perm.can_read {
                return Err(DomainError::NotFound(tid));
            }
        }
        _ => return Err(DomainError::Validation("非法的关注对象".into())),
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/follows",
    tag = "社区/关注",
    description = "关注对象（target_type 只有 user|forum|topic 三档）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/follows")]
pub(crate) async fn follow_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FollowReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let ttype = normalize_follow_type(&body.target_type)
        .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?;
    validate_follow_target(&state.repo.db, &auth, ttype, body.target_id)
        .await?;
    sqlx::query(
        "INSERT INTO follows (user_id, target_type, target_id) VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, target_type, target_id) DO NOTHING",
    )
    .bind(auth.id)
    .bind(ttype)
    .bind(body.target_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND \
         target_id = $2",
    )
    .bind(ttype)
    .bind(body.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "target_type": ttype,
        "target_id": body.target_id,
        "following": true,
        "followers": followers,
    })))
}
#[utoipa::path(
    delete,
    path = "/api/v1/follows/{ttype}/{tid}",
    tag = "社区/关注",
    description = "取消关注。",
    params(
        ("ttype" = String, description = "关注类型"),
        ("tid" = i64, description = "目标 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[delete("/follows/{ttype}/{tid}")]
pub(crate) async fn follow_delete(
    req: HttpRequest,
    path: web::Path<(String, i64)>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (raw_type, tid) = path.into_inner();
    let ttype = normalize_follow_type(&raw_type)
        .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?;
    sqlx::query(
        "DELETE FROM follows WHERE user_id = $1 AND target_type = \
     $2 AND target_id = $3",
    )
    .bind(auth.id)
    .bind(ttype)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND \
         target_id = $2",
    )
    .bind(ttype)
    .bind(tid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "target_type": ttype,
        "target_id": tid,
        "following": false,
        "followers": followers,
    })))
}

//! M15 关注订阅（用户/版块/主题 + 关注流）。
//! 从 community_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn normalize_follow_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "user" => Some("user"),
        "forum" => Some("forum"),
        "topic" => Some("topic"),
        _ => None,
    }
}

#[derive(Deserialize)]
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
                "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status < 2)",
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

#[post("/follows")]
async fn follow_create(
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
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
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

#[delete("/follows/{ttype}/{tid}")]
async fn follow_delete(
    req: HttpRequest,
    path: web::Path<(String, i64)>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (raw_type, tid) = path.into_inner();
    let ttype = normalize_follow_type(&raw_type)
        .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?;
    sqlx::query("DELETE FROM follows WHERE user_id = $1 AND target_type = $2 AND target_id = $3")
        .bind(auth.id)
        .bind(ttype)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
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

#[derive(Deserialize)]
struct FollowStatusQuery {
    target_type: String,
    target_id: i64,
}

/// 关注状态。用户页无当前登录者信息，故 `is_self` 也在这里回给前端，由组件决定隐藏按钮。
#[get("/follows/status")]
async fn follow_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FollowStatusQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let Some(ttype) = normalize_follow_type(&q.target_type) else {
        return Err(DomainError::Validation("非法的关注对象".into()));
    };
    let following: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM follows WHERE user_id = $1 AND target_type = $2 AND target_id = $3)",
    )
    .bind(auth.id)
    .bind(ttype)
    .bind(q.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
    )
    .bind(ttype)
    .bind(q.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "following": following,
        "followers": followers,
        "is_self": ttype == "user" && q.target_id == auth.id,
    })))
}

#[derive(Deserialize)]
struct FollowMineQuery {
    #[serde(default)]
    target_type: Option<String>,
}

/// 我的关注列表。只关注用户时返回 username，关注版块返回 name，关注主题返回 title——
/// 一次 JOIN 三张表反而更绕，改成按 type 分别取再合并（数量级很小）。
#[get("/follows/mine")]
async fn follow_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FollowMineQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let want =
        match q.target_type.as_deref() {
            Some(s) => Some(normalize_follow_type(s).ok_or_else(|| {
                DomainError::Validation("非法的关注对象".into())
            })?),
            None => None,
        };
    let mut users: Vec<serde_json::Value> = Vec::new();
    let mut forums: Vec<serde_json::Value> = Vec::new();
    let mut topics: Vec<serde_json::Value> = Vec::new();

    if want.is_none() || want == Some("user") {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT u.id, u.username FROM follows f JOIN users u ON u.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'user' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        users = rows
            .into_iter()
            .map(|(id, username)| serde_json::json!({ "id": id, "name": username }))
            .collect();
    }
    if want.is_none() || want == Some("forum") {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT fc.id, fc.name FROM follows f JOIN forums fc ON fc.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'forum' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        forums = rows
            .into_iter()
            .map(|(id, name)| serde_json::json!({ "id": id, "name": name }))
            .collect();
    }
    if want.is_none() || want == Some("topic") {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT t.id, t.title, t.forum_id FROM follows f JOIN topics t ON t.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'topic' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        topics = rows
            .into_iter()
            .map(|(id, title, forum_id)| serde_json::json!({ "id": id, "name": title, "forum_id": forum_id }))
            .collect();
    }
    Ok(ok(serde_json::json!({
        "users": users, "forums": forums, "topics": topics,
    })))
}

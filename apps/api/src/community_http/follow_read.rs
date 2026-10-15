//! 关注状态与我的关注（/follows/status、/follows/mine）。
//! follow.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::follows（H 层不 import
//! sqlx，FRAME §2）。本文件只留鉴权、口径判断与 JSON 组装。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::follows as repo_follows;
use crate::state::AppState;

use super::follow::normalize_follow_type;

#[derive(Deserialize)]
pub(crate) struct FollowStatusQuery {
    pub(crate) target_type: String,
    pub(crate) target_id: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/follows/status",
    tag = "社区/关注",
    description = "关注状态批量查询（供列表页标星）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/follows/status")]
/// 关注状态。用户页无当前登录者信息，故 `is_self` 也在这里回给前端，由组件决定隐藏按钮。
pub(crate) async fn follow_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<FollowStatusQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let Some(ttype) = normalize_follow_type(&q.target_type) else {
        return Err(DomainError::Validation("非法的关注对象".into()));
    };
    let following = repo_follows::is_following(
        &state.repo,
        auth.id,
        ttype,
        q.target_id,
    )
    .await;
    let followers =
        repo_follows::follower_count(&state.repo, ttype, q.target_id).await;
    Ok(ok(serde_json::json!({
        "following": following,
        "followers": followers,
        "is_self": ttype == "user" && q.target_id == auth.id,
    })))
}

#[utoipa::path(
    get,
    path = "/api/v1/follows/mine",
    tag = "社区/关注",
    description = "我的关注列表（按类型分组）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/follows/mine")]
/// 我的关注列表。只关注用户时返回 username，关注版块返回 name，关注主题返回 title——
/// 一次 JOIN 三张表反而更绕，改成按 type 分别取再合并（数量级很小）。
pub(crate) async fn follow_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<FollowMineQuery>,
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
        users = repo_follows::my_user_follows(&state.repo, auth.id)
            .await?
            .into_iter()
            .map(|(id, username)| {
                serde_json::json!({ "id": id, "name": username })
            })
            .collect();
    }
    if want.is_none() || want == Some("forum") {
        forums = repo_follows::my_forum_follows(&state.repo, auth.id)
            .await?
            .into_iter()
            .map(|(id, name)| serde_json::json!({ "id": id, "name": name }))
            .collect();
    }
    if want.is_none() || want == Some("topic") {
        topics = repo_follows::my_topic_follows(&state.repo, auth.id)
            .await?
            .into_iter()
            .map(|(id, title, forum_id)| {
                serde_json::json!({
                    "id": id, "name": title, "forum_id": forum_id,
                })
            })
            .collect();
    }
    Ok(ok(serde_json::json!({
        "users": users, "forums": forums, "topics": topics,
    })))
}

#[derive(Deserialize)]
pub(crate) struct FollowMineQuery {
    #[serde(default)]
    pub(crate) target_type: Option<String>,
}

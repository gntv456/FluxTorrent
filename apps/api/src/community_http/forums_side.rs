//! 论坛首页右栏聚合（GET /forums/side）：论坛统计 + 今日热帖 + 最新回复。
//!
//! 从 forums.rs 按域拆出（forums.rs 已 286 行，逼近 300 行门禁）；
//! SQL 全部住在 repo::community::forum_side（H 层零容忍带 SQL）。
//!
//! 样图 § tplForum 右栏四块里，「新手必读」是静态文案（走 i18n，不进
//! 接口）；「公告」这一档本站没有数据模型（topics.topic_type 只有
//! bounty/poll/lottery），故不在此伪造。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::forum_side as repo_side;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, Responder};

#[utoipa::path(
    get,
    path = "/api/v1/forums/side",
    tag = "社区/论坛",
    description = "论坛首页右栏：统计 / 今日热帖 / 最新回复（均按读权限过滤）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/forums/side")]
pub(crate) async fn forum_side(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (topics, posts, today) =
        repo_side::side_stats(&state.repo, auth.class_id).await?;
    let hot = repo_side::hot_topics(&state.repo, auth.class_id).await?;
    let latest = repo_side::latest_replies(&state.repo, auth.class_id).await?;
    Ok(ok(serde_json::json!({
        "stats": { "topics": topics, "posts": posts, "today_topics": today },
        "hot": hot,
        "latest_replies": latest,
    })))
}

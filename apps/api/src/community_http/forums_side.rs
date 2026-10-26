//! 论坛首页聚合（GET /forums/side 右栏 + GET /forums/recent 中栏话题流）。
//!
//! 从 forums.rs 按域拆出（forums.rs 已 286 行，逼近 300 行门禁）；
//! SQL 全部住在 repo::community::forum_side（H 层零容忍带 SQL）。
//!
//! 样图 § tplForum 右栏四块里，「新手必读」是静态文案（走 i18n，不进
//! 接口）；「公告」这一档本站没有数据模型（topics.topic_type 只有
//! bounty/poll/lottery），故不在此伪造。中栏话题流（/forums/recent）
//! 与右栏同源同权限口径，只是取「跨版块最新主题」而非聚合榜。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::forum_side as repo_side;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

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

#[derive(Deserialize)]
struct RecentQuery {
    /// 类型筛选（对齐样图筛选行四档）：notice=只看公告 / sticky=只看置顶 /
    /// digest=只看精华，缺省全量。白名单之外一律回落全量。
    #[serde(default)]
    r#type: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/forums/recent",
    tag = "社区/论坛",
    description = "论坛首页话题流：全站可读版块的最新主题（版块徽章 + 最后回复者）。",
    params(
        ("type" = Option<String>, description = "sticky=只看置顶 / digest=只看精华"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/forums/recent")]
pub(crate) async fn forum_recent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<RecentQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 白名单校验（H 层）：非法值回落全量，不拿 400 打断浏览
    let only = match q.r#type.as_deref() {
        Some("notice") => Some("notice"),
        Some("sticky") => Some("sticky"),
        Some("digest") => Some("digest"),
        _ => None,
    };
    // 热度标阈值走管理端设置（0397）：0 = 关闭，缺省 1000
    let hot_min = repo_side::hot_views_threshold(&state.repo).await;
    let topics = repo_side::recent_topics(
        &state.repo,
        auth.class_id,
        auth.id,
        auth.class_id >= 93,
        only,
        hot_min,
    )
    .await?;
    Ok(ok(serde_json::json!({ "topics": topics })))
}

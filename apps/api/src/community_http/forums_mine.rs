//! 我参与的论坛活动（GET /forums/mine）：我发的主题 / 我发的回复。
//!
//! SQL 全部住在 repo::community::forum_mine（H 层零容忍带 SQL）。
//! 分页口径与版块主题列表一致：page/per/total，越界页回空数组但
//! total 如实报（前端据此画页码）。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::forum_mine as repo_mine;
use crate::state::AppState;

/// 每页条数：两块列表共用，前端提示语也回显这个数。
pub(crate) const MINE_PAGE_SIZE: i64 = 20;

#[derive(Deserialize)]
struct MineQuery {
    /// topics=我发的主题（缺省）/ replies=我的回复；其余值回落 topics
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    page: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/forums/mine",
    tag = "社区/论坛",
    description = "我参与的主题与回复（按读权限过滤，分页）。",
    params(
        ("type" = Option<String>, description = "topics|replies，缺省 topics"),
        ("page" = Option<i64>, description = "页码，1 起，每页 20 条"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/forums/mine")]
pub(crate) async fn forum_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<MineQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let per = MINE_PAGE_SIZE;
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let offset = (page - 1) * per;
    let kind = if q.r#type.as_deref() == Some("replies") {
        "replies"
    } else {
        "topics"
    };
    let (total, items) = if kind == "replies" {
        (
            repo_mine::count_my_replies(
                &state.repo,
                auth.id,
                auth.class_id,
            )
            .await?,
            serde_json::json!(
                repo_mine::my_replies(
                    &state.repo,
                    auth.id,
                    auth.class_id,
                    per,
                    offset,
                )
                .await?
            ),
        )
    } else {
        (
            repo_mine::count_my_topics(
                &state.repo,
                auth.id,
                auth.class_id,
            )
            .await?,
            serde_json::json!(
                repo_mine::my_topics(
                    &state.repo,
                    auth.id,
                    auth.class_id,
                    per,
                    offset,
                )
                .await?
            ),
        )
    };
    Ok(ok(serde_json::json!({
        "type": kind,
        "page": page,
        "per": per,
        "total": total,
        "items": items,
    })))
}

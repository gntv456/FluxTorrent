//! 种子标签读写（T-04/0159 标签体系）：列表 + 覆盖写。
//! 从 interact.rs 按域拆出（300 行门禁）。

use actix_web::{get, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;
/// 种子标签（T-04）：字典 + 已打
#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/tags",
    tag = "种子",
    description = "种子标签列表（0159 标签体系）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/tags")]
pub(crate) async fn torrent_tags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let viewer = (auth.id, auth.class_id >= 90);
    Ok(ok(torrents::list_tags(&state.repo.db, id, viewer).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct TagReq {
    tag_id: i32,
    on: bool,
}

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}/tags",
    tag = "种子",
    description = "覆盖写标签（0159；any/all 两种匹配口径由 tag_all 决定）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[put("/torrents/{id}/tags")]
pub(crate) async fn torrent_tag_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TagReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::tag_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        body.tag_id,
        body.on,
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.tag", Some(tid))
        .await;
    // 聚合缓存：详情页标签条读的就是 aggregate.tags.mine——不失效的话，
    // 用户点完「官种/中字」刷新页面标签条纹丝不动（2026-10-10 实测）
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "tag": body.tag_id, "on": body.on })))
}

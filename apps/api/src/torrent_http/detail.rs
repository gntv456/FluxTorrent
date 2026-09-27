//! 种子详情只读面（M03）：详情/扩展详情/文件树/感谢者/评论读取/抓取列表/NFO。
//! 从 torrent_http/list.rs 按域拆出（列表与查询助手留在 list.rs）。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::query::ListQuery;

#[get("/torrents/{id}")]
async fn detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 持 view_anonymous 权限者可见匿名种子的真实发布者
    let reveal = crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::TORRENT_VIEW_ANONYMOUS,
    )
    .await;
    // G7：staff 视角传 (uid, true)——暂缓种对 staff 开放
    let viewer = (auth.id, auth.class_id >= 90);
    let t = torrents::get_torrent(
        &state.repo.read_db,
        path.into_inner(),
        reveal,
        Some(viewer),
    )
    .await?;
    Ok(ok(t))
}

/// 详情页扩展数据（简介/文件数/感谢数），与 detail 合并渲染
#[get("/torrents/{id}/detail")]
async fn torrent_detail_ext(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let t = torrents::get_torrent_detail(
        &state.repo.read_db,
        path.into_inner(),
        auth.id,
    )
    .await?;
    Ok(ok(t))
}

#[get("/torrents/{id}/files")]
async fn torrent_files(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::list_files(
        &state.repo.read_db,
        path.into_inner(),
    )
    .await?))
}

#[get("/torrents/{id}/thanks")]
async fn torrent_thanks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::list_thanks(
        &state.repo.read_db,
        path.into_inner(),
    )
    .await?))
}

#[get("/torrents/{id}/comments")]
async fn comments(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let items = torrents::list_comments_as(
        &state.repo.read_db,
        path.into_inner(),
        q.limit.unwrap_or(20),
        auth.id,
    )
    .await?;
    Ok(ok(items))
}

#[derive(Deserialize)]
pub(super) struct CommentReq {
    pub(super) body: String,
    /// 嵌套回复（0156）：被回复的评论 id；缺省 = 顶层评论
    #[serde(default)]
    pub(super) parent_id: Option<i64>,
}

/// 下载/做种记录（NP viewsnatches.php）
#[get("/torrents/{id}/snatches")]
async fn torrent_snatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::list_snatches(
        &state.repo.read_db,
        path.into_inner(),
    )
    .await?))
}

/// NFO（NP viewnfo.php）
#[get("/torrents/{id}/nfo")]
async fn torrent_nfo(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let nfo = torrents::get_nfo(&state.repo.read_db, path.into_inner()).await?;
    Ok(ok(serde_json::json!({ "nfo": nfo })))
}

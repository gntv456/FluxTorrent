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

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}",
    tag = "种子",
    description = "种子详情（会话鉴权）。持 view_anonymous 权限者可见匿名种子的真实发布者；staff\
            对暂缓/待审种也可见。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}")]
pub(crate) async fn detail(
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

/// 详情页扩展数据（简介/文件数/感谢数），与 detail 合并渲染。
/// 0417 口径澄清（VIP 深测 F2）：本端点是 GET /torrents/{id} 的**补充段**
/// （descr/numfiles/thanks_count/sections…），核心字段（name/size/seeders）
/// 在主端点；对外深详情走 GET /open/torrents/{id}（两者已合并）。为防
/// 第三方按直觉单接本端点渲染出「无标题卡片」，name/size 在此冗余回显。
#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/detail",
    tag = "种子",
    description = "详情页扩展数据（简介/文件数/感谢数），与详情合并渲染。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/detail")]
pub(crate) async fn torrent_detail_ext(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 审查修正（P1-9）：主查钉主库——purchased/is_owner 是写后立读字段
    // （购买/发布跳转立即刷新详情），副本延迟会显示未购买；files/thanks/
    // comments 等纯展示段保持 read_db。
    let t = torrents::get_torrent_detail(
        &state.repo.db,
        path.into_inner(),
        auth.id,
    )
    .await?;
    Ok(ok(t))
}

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/files",
    tag = "种子",
    description = "文件树（按目录层级返回，付费种需已购或为发布者本人）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/files")]
pub(crate) async fn torrent_files(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 与主详情同一准入闸门（0288 / P1-1）：未过审种子的附属数据不再各开各的口子
    torrents::assert_visible(
        &state.repo.read_db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    Ok(ok(torrents::list_files(&state.repo.read_db, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/thanks",
    tag = "种子",
    description = "感谢者列表（分页）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/thanks")]
pub(crate) async fn torrent_thanks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 与主详情同一准入闸门（0288 / P1-1）：未过审种子的附属数据不再各开各的口子
    torrents::assert_visible(
        &state.repo.read_db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    Ok(ok(torrents::list_thanks(&state.repo.read_db, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/comments",
    tag = "种子",
    description = "评论列表（分页，含点赞数与楼中楼计数）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/comments")]
pub(crate) async fn comments(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: crate::http::Q<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    torrents::assert_visible(
        &state.repo.read_db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    let items = torrents::list_comments_as(
        &state.repo.read_db,
        id,
        q.limit.unwrap_or(20),
        auth.id,
    )
    .await?;
    Ok(ok(items))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(super) struct CommentReq {
    pub(super) body: String,
    /// 嵌套回复（0156）：被回复的评论 id；缺省 = 顶层评论
    #[serde(default)]
    pub(super) parent_id: Option<i64>,
}

/// 下载/做种记录（NP viewsnatches.php）
#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/snatches",
    tag = "种子",
    description = "抓取列表（做种/下载两种视角，按 viewer 过滤）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/snatches")]
pub(crate) async fn torrent_snatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 与主详情同一准入闸门（0288 / P1-1）：未过审种子的附属数据不再各开各的口子
    torrents::assert_visible(
        &state.repo.read_db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    Ok(ok(torrents::list_snatches(&state.repo.read_db, id).await?))
}

/// NFO（NP viewnfo.php）
#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/nfo",
    tag = "种子",
    description = "NFO 原文（text/plain）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/nfo")]
pub(crate) async fn torrent_nfo(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 审核员要能读待审/被拒种子的 NFO（判压制来源与是否盗版的主要依据，旧口径 404）
    let nfo = torrents::get_nfo(
        &state.repo.read_db,
        path.into_inner(),
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    Ok(ok(serde_json::json!({ "nfo": nfo })))
}

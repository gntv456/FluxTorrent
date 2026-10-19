//! 种子互动（M07/M10）：发评论/删评论/感谢/收藏/求续种/标签读写。
//! 从 torrent_http.rs 按域拆出。

use actix_web::{
    delete, get, post, put, web, HttpRequest, HttpResponse, Responder,
};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::detail::CommentReq;

#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/comments",
    tag = "种子",
    description = "发评论。触发门槛：注册满 N 天、魔力/做种量达标（见站规）。",
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
#[post("/torrents/{id}/comments")]
pub(crate) async fn create_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CommentReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let id = torrents::add_comment_as(
        &state.repo.db,
        tid,
        auth.id,
        &body.body,
        body.parent_id,
    )
    .await?;
    // 详情页聚合缓存（2.5）：评论数/评论列表是 aggregate 的一部分，
    // 写完不失效就会端着旧快照（与打标/感谢同一批漏项）
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// 审计修复（P1）：评论此前只有创建+列表，作者本人与版主都无法删除（无任何删除端点）。
/// 作者本人或持 torrent.manage 的 staff 可删；挂审计日志。
#[utoipa::path(
    delete,
    path = "/api/v1/torrents/{id}/comments/{cid}",
    tag = "种子",
    description = "删评论（作者本人或持 forum 删帖权）。",
    params(
        ("id" = i64, description = "种子 id"),
        ("cid" = i64, description = "评论 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[delete("/torrents/{id}/comments/{cid}")]
pub(crate) async fn delete_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (torrent_id, comment_id) = path.into_inner();
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM comments WHERE id = $1 AND torrent_id = $2",
    )
    .bind(comment_id)
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(comment_id));
    };
    let is_owner = owner_id == auth.id;
    let is_manager =
        crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_MANAGE)
            .await;
    if !is_owner && !is_manager {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(comment_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(
            Some(auth.id),
            if is_owner {
                "comment.delete_own"
            } else {
                "comment.delete_staff"
            },
            Some(comment_id),
        )
        .await;
    // 聚合缓存：评论数与评论列表都在 aggregate 里
    super::aggregate::invalidate_tdetail_cache(&state, torrent_id).await;
    Ok(ok(serde_json::json!({ "deleted": comment_id })))
}

/// 评论点赞切换（0155 评论增强）：已赞 → 取消；未赞 → 点赞。
/// 不许赞自己的评论（错误信息友好，竞品通行口径）；并发幂等由 PK 兜底。
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/comments/{cid}/like",
    tag = "种子",
    description = "评论点赞切换（返回切换后的点赞数与当前状态）。",
    params(
        ("id" = i64, description = "种子 id"),
        ("cid" = i64, description = "评论 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/torrents/{id}/comments/{cid}/like")]
pub(crate) async fn toggle_comment_like(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (torrent_id, comment_id) = path.into_inner();
    let author: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM comments WHERE id = $1 AND torrent_id = $2",
    )
    .bind(comment_id)
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(author_id) = author else {
        return Err(DomainError::NotFound(comment_id));
    };
    if author_id == auth.id {
        return Err(DomainError::Validation("不能给自己的评论点赞".into()));
    }
    let db = &state.repo.db;
    let liked =
        super::comments_like::toggle_like(db, comment_id, auth.id).await?;
    let likes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM comment_likes WHERE comment_id = $1",
    )
    .bind(comment_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 聚合缓存：aggregate.comments 带着每条评论的点赞数
    super::aggregate::invalidate_tdetail_cache(&state, torrent_id).await;
    Ok(ok(serde_json::json!({ "liked": liked, "likes": likes })))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct ThankBody {
    /// 魔力答谢数额（馒头口径：+1/+10/+100/+500/+1000/+10000；缺省 0 = 免费感谢）
    #[serde(default)]
    amount: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/thanks",
    tag = "种子",
    description = "感谢（已感谢过返回 code 5002）。",
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
#[post("/torrents/{id}/thanks")]
pub(crate) async fn do_thank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: Option<web::Json<ThankBody>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let amount = body.and_then(|b| b.amount).unwrap_or(0);
    if amount > 0 && !super::thanks_spark::is_valid_amount(amount) {
        return Err(DomainError::Validation(
            "答谢数额需为 1/10/100/500/1000/10000".into(),
        ));
    }
    // 种主随种一起读出；种不存在直接 404（旧版 owner=None 时静默跳过转账）
    let owner = super::thanks_spark::owner_of(&state.repo.db, tid).await?;
    if amount > 0 && owner == auth.id {
        return Err(DomainError::Validation("不能向自己的种子答谢魔力".into()));
    }
    // P-360/380：感谢行与转账同事务；键取本次行时间戳（成因见 thanks_spark）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row_ts = torrents::thank(&mut *tx, tid, auth.id).await?;
    let given = if amount > 0 {
        super::thanks_spark::transfer(&mut tx, auth.id, owner, amount, tid,
                                      row_ts)
            .await?
    } else {
        0
    };
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 聚合缓存：thanks_count 与「最近活动」都在 aggregate 里
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "thanked": true, "spark_given": given })))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct BookmarkReq {
    on: bool,
}

/// 编辑种子（NP edit/takeedit 作者口径；修改后回退待审）
#[derive(Deserialize, utoipa::ToSchema)]
pub(super) struct TorrentEditReq {
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) small_descr: Option<String>,
    #[serde(default)]
    pub(super) descr: Option<String>,
    #[serde(default)]
    pub(super) anonymous: Option<bool>,
    #[serde(default)]
    pub(super) category_id: Option<i32>,
    #[serde(default)]
    pub(super) medium_id: Option<i32>,
    #[serde(default)]
    pub(super) grade_id: Option<i32>,
    #[serde(default)]
    pub(super) edition_id: Option<i32>,
    /// IMDB id（0150：后台/编辑表单直填；非法形态会被 manage 层过滤为 None）
    #[serde(default)]
    pub(super) imdb_id: Option<String>,
    /// 封面外链（0173 编辑同步发布能力）：None=不动，Some("")=清除，Some(url)=写入
    #[serde(default)]
    pub(super) poster: Option<String>,
    /// MediaInfo 全文（0173 编辑同步发布能力）：None=不动，Some("")=清除，Some(text)=写入
    #[serde(default)]
    pub(super) mediainfo: Option<String>,
    /// NFO 全文（同上三态）：此前 nfo 只有发布口能写，传错无法改
    #[serde(default)]
    pub(super) nfo: Option<String>,
    /// 标签（0159 P1）：编辑表单整组提交；Some([]) = 清空全部标签，
    /// None = 不动（编辑入口与详情页 toggle / 发布同走 apply_torrent_tags）
    #[serde(default)]
    pub(super) tag_ids: Option<Vec<i32>>,
    /// 多维属性（B2 六类型）：{kind: 值}；缺省 = 不动。
    /// 值可以是旧格式整数（枚举单选）或新格式对象（详见
    /// `publish_http::upload_sections` 的协议说明）。提交即整组重建
    /// （未含的旧维删除——与前端「整表单保存」语义一致）。
    #[serde(default)]
    pub(super) sections: Option<serde_json::Value>,
    /// 推荐位（0184 编辑对齐发布页）：staff 专属；None = 不动
    #[serde(default)]
    pub(super) pos_state: Option<i16>,
    /// 置顶截止（RFC3339；空串 = 清除）
    #[serde(default)]
    pub(super) pos_state_until: Option<String>,
    /// 推荐影片（0/1/2；None = 不动）
    #[serde(default)]
    pub(super) pick_type: Option<i16>,
}

/// 请求补种（NP takereseed.php：死种 → PM 全体完成者，900s 限频）
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/reseed",
    tag = "种子",
    description = "求种（挂出求种位；过期天数 <=0 走 30 天兜底，避免悬赏永久冻结）。",
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
#[post("/torrents/{id}/reseed")]
pub(crate) async fn request_reseed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_else(|_| "user".into());
    let n = torrents::request_reseed(&state.repo.db, tid, (auth.id, username))
        .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.reseed", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "notified": n })))
}

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

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}/bookmark",
    tag = "种子",
    description = "收藏切换。",
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
#[put("/torrents/{id}/bookmark")]
pub(crate) async fn do_bookmark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<BookmarkReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::bookmark(&state.repo.db, tid, auth.id, body.on).await?;
    // 聚合缓存：bookmark_count 在 aggregate.detail 里
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "bookmarked": body.on })))
}

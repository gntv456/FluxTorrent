//! 种子互动（M07/M10）：发评论/删评论/感谢/收藏/求续种/标签读写。
//! 从 torrent_http.rs 按域拆出。

use actix_web::{delete, post, put, web, HttpRequest, HttpResponse, Responder};
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
    // 评论通知无去重：按人 12 条/分（一次讨论远低于此），放在内容校验/
    // 落库之前——单人即可把种主信箱灌满，洪水最贵的不止那行 INSERT。
    crate::http::throttle_limited(&state, format!("cmt:{}", auth.id), 12)
        .await?;
    let tid = path.into_inner();
    let id = torrents::add_comment_as(
        &state.repo.db,
        tid,
        auth.id,
        &body.body,
        body.parent_id,
    )
    .await?;
    // 0503 死奖励卡接线：addcomment（NP 行为奖励，后台可调，0=关）——
    // 每条评论一笔，幂等锚评论 id（删评不追回，NP 同口径）
    let cmt_r: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'addcomment'), 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if cmt_r > 0 {
        let idem = format!("addcomment:{}", id);
        if let Err(e) = crate::economy_http::earn_spark(
            &state.repo.db,
            auth.id,
            cmt_r,
            "addcomment",
            &idem,
        )
        .await
        {
            tracing::warn!(%e, cid = id, "评论奖励入账失败");
        }
    }
    // 0423 发布者正反馈：通知种主（详见 notify.rs）
    super::notify::notify_comment(&state.repo.db, tid, auth.id).await;
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
    let action = if is_owner {
        "comment.delete_own"
    } else {
        "comment.delete_staff"
    };
    let _ = state
        .repo
        .audit(Some(auth.id), action, Some(comment_id))
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
    // 0288 唯一可见性口径：主详情 404 的种子，点赞口不能再单独放行
    torrents::assert_visible(
        &state.repo.db,
        torrent_id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
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
    let likes: i64 = super::comments_like::like_count(db, comment_id).await?;
    // 聚合缓存：aggregate.comments 带着每条评论的点赞数
    super::aggregate::invalidate_tdetail_cache(&state, torrent_id).await;
    Ok(ok(serde_json::json!({ "liked": liked, "likes": likes })))
}

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
    /// 豆瓣条目（编辑对齐发布页 metaIds）：None=不动；Some=按发种同款
    /// 提取器归一，空串=清除（manage 层分派）
    #[serde(default)]
    pub(super) douban: Option<String>,
    /// TMDB 条目（同上）：manage 层归一后与 douban 同写 media_info
    #[serde(default)]
    pub(super) tmdb: Option<String>,
    /// 发布者 H&R 免罪分成开关（0424 单向 opt-in）：None=不动；
    /// Some(false)=只清 publisher_enabled 键，Some(true)=写入
    #[serde(default)]
    pub(super) hr_punish: Option<bool>,
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

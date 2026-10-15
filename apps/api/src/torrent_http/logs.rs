//! 抓轨日志正文端点（0312：详情页「查看日志」）。
//!
//! 正文不进 aggregate 首屏（单碟 EAC 日志可上百 KiB，多碟会把共享段缓存体积
//! 打爆），点开才取；可见性由 `torrents::get_log` 内的统一口径判定。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/logs/{ordinal}",
    tag = "种子",
    description = "抓轨日志正文（0312；ordinal 为该种日志序号）。",
    params(
        ("id" = i64, description = "种子 id"),
        ("ordinal" = i64, description = "日志序号"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/logs/{ordinal}")]
pub(crate) async fn torrent_log_body(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i32)>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (id, ordinal) = path.into_inner();
    let viewer = (auth.id, auth.class_id >= 90);
    let hit = torrents::get_log(&state.repo.db, id, ordinal, viewer).await?;
    let Some((meta, body)) = hit else {
        return Err(DomainError::NotFound(id));
    };
    Ok(ok(serde_json::json!({
        "torrent_id": id,
        "ordinal": meta.ordinal,
        "filename": meta.filename,
        "engine": meta.engine,
        "log_score": meta.log_score,
        "tracks": meta.tracks,
        "issues": meta.issues,
        "size": meta.size,
        "body": body,
    })))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AdjustLogReq {
    /// 空 = 撤销改判，回到引擎判分
    #[serde(default)]
    score: Option<i16>,
    #[serde(default)]
    reason: Option<String>,
}

/// 版主人工改判日志分（0337，Gazelle `AdjustedScore` 口径）。
/// 原始 `log_score` 不动，改判写 `adjusted_*` 四列（可审计）。
#[utoipa::path(
    post,
    path = "/api/v1/admin/torrents/{id}/logs/{ordinal}/adjust",
    tag = "种子",
    description = "抓轨日志人工修正（staff；补记/更正漏抓的 announce 事件）。",
    params(
        ("id" = i64, description = "种子 id"),
        ("ordinal" = i64, description = "日志序号"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/admin/torrents/{id}/logs/{ordinal}/adjust")]
pub async fn log_adjust(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i32)>,
    body: web::Json<AdjustLogReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let (id, ordinal) = path.into_inner();
    let score = body.score.map(|s| s.clamp(0, 100));
    let reason = body.reason.as_deref().unwrap_or("").trim();
    if reason.chars().count() > 200 {
        return Err(DomainError::Validation("改判理由需 ≤200 字".into()));
    }
    let hit = torrents::adjust_log(
        &state.repo.db,
        id,
        ordinal,
        score,
        auth.id,
        reason,
    )
    .await?;
    if !hit {
        return Err(DomainError::NotFound(id));
    }
    // 改判改变有效分 → 筛选维度（log_score/haslog）随之回流（0314 补全）
    torrents::sync_log_dims(&state.repo.db, id).await?;
    // 公信动作进审计日志（本仓 admin 写操作的留痕口径）
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "torrent.log_adjust",
            Some(id),
            Some(reason),
            Some(serde_json::json!({
                "ordinal": ordinal, "adjusted_score": score,
            })),
        )
        .await;
    // 改判是**判定口径**的变更：不清详情缓存的话，版主裁完分数在页面上
    // 还要旧 30 秒（manage.rs 五个写口都清，0337 这条漏了）
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({
        "torrent_id": id, "ordinal": ordinal, "adjusted_score": score,
    })))
}

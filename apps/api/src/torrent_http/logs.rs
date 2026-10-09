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

#[get("/torrents/{id}/logs/{ordinal}")]
async fn torrent_log_body(
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

#[derive(serde::Deserialize)]
pub(crate) struct AdjustLogReq {
    /// 空 = 撤销改判，回到引擎判分
    #[serde(default)]
    score: Option<i16>,
    #[serde(default)]
    reason: Option<String>,
}

/// 版主人工改判日志分（0337，Gazelle `AdjustedScore` 口径）。
/// 原始 `log_score` 不动，改判写 `adjusted_*` 四列（可审计）。
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

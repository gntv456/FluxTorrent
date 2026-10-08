//! 抓轨日志正文端点（0312：详情页「查看日志」）。
//!
//! 正文不进 aggregate 首屏（单碟 EAC 日志可上百 KiB，多碟会把共享段缓存体积
//! 打爆），点开才取；可见性由 `torrents::get_log` 内的统一口径判定。

use actix_web::{get, web, HttpRequest, HttpResponse};

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

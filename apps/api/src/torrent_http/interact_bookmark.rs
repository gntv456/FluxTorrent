//! bookmark DELETE 对称口（从 interact.rs 拆出，行数门禁）。

use actix_web::{delete, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

/// DELETE 对称口（0417 F5）：REST 客户端/第三方工具按直觉调
/// DELETE /torrents/{id}/bookmark 取消收藏（旧口径只有 PUT {on:false}，
/// 实测 404）。语义与 PUT on=false 完全一致：取消不设可见性闸门
/// （种变不可见后仍要能清掉这一行），幂等（无行删 0 也回 false）。
#[utoipa::path(
    delete,
    path = "/api/v1/torrents/{id}/bookmark",
    tag = "种子",
    description = "取消收藏（与 PUT {on:false} 同义；幂等）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在（code 1004）"),
    )
)]
#[delete("/torrents/{id}/bookmark")]
pub(crate) async fn delete_bookmark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::bookmark(&state.repo.db, tid, auth.id, false).await?;
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "bookmarked": false })))
}

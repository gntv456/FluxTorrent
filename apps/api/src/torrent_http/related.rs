//! 相似推荐端点（2026-10-08 二轮实证缺口 P1）：
//! GET /torrents/{id}/related?limit=8
//!
//! 详情页"相关种子"位。打分逻辑在 `torrents::related`（同组 > 同标签 > 同分类）。
//! 只读、无用户维度，登录即可用；口径与列表一致（approval_status=1）。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

#[derive(serde::Deserialize)]
pub struct RelatedQuery {
    limit: Option<i64>,
}

#[get("/torrents/{id}/related")]
pub async fn torrent_related(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<RelatedQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let limit = q.limit.unwrap_or(8);
    let items = torrents::list_related(&state.repo.read_db, id, limit).await?;
    Ok(ok(serde_json::json!({ "items": items })))
}

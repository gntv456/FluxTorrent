//! 赛事详情（GET /matches/{id}）从 sport_matches.rs 按域拆出。
//! sport_matches.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::matches（H 层不 import
//! sqlx，FRAME §2）。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::matches as repo_matches;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

/// 对阵页：该场全部过审种（全场/集锦/回放并列）。
#[utoipa::path(
    get,
    path = "/api/v1/matches/{id}",
    tag = "社区/赛事",
    description = "赛事详情（含对阵与比分）。",
    params(
        ("id" = i64, description = "赛事 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/matches/{id}")]
pub async fn match_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let Some(m) = repo_matches::match_json(&state.repo, id).await? else {
        return Err(DomainError::NotFound(id));
    };
    let items = repo_matches::match_items(&state.repo, id).await?;
    Ok(ok(serde_json::json!({
        "match": m,
        "items": items.iter().map(|(tid, n, sz, sd, tc)| {
            serde_json::json!({
                "id": tid, "name": n, "size": sz,
                "seeders": sd, "times_completed": tc,
            })
        }).collect::<Vec<_>>(),
    })))
}

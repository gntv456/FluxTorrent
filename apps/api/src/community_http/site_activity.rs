//! 全站动态时间线（0407）：系统事件流的读口。三渠分立口径见迁移 0407
//! 头注释（公告=管理员发布 / 聊天室=用户聊天 / 动态=系统事件）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Serialize, sqlx::FromRow)]
pub(crate) struct ActivityRow {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(
    get,
    path = "/api/v1/site-activity",
    tag = "社区/全站动态",
    description = "全站动态时间线（字幕上架/评选揭晓等系统事件，最近 50 条，需登录）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/site-activity")]
pub(crate) async fn site_activity_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let rows: Vec<ActivityRow> = sqlx::query_as(
        "SELECT id, kind, text, created_at FROM site_activity          ORDER BY id DESC LIMIT 50",
    )
    .bind(0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

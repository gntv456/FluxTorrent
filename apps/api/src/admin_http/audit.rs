use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
use super::user_status::SearchQ;

// ============ 审计日志 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AuditRow {
    id: i64,
    actor_id: Option<i64>,
    action: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/audit")]
async fn audit_query(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SearchQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AUDIT_VIEW)
        .await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, actor_id, action, created_at FROM audit_log \
         WHERE action ILIKE $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(pattern)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

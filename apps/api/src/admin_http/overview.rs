use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 运营概览：待审/举报/用户/种子计数
#[get("/admin/overview")]
async fn admin_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (pending, reports, users, torrents, banned): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
            (SELECT count(*) FROM torrents WHERE approval_status = 0), \
            (SELECT count(*) FROM reports WHERE status = 0), \
            (SELECT count(*) FROM users), \
            (SELECT count(*) FROM torrents), \
            (SELECT count(*) FROM users WHERE status >= 2)",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "pending_reviews": pending, "open_reports": reports,
        "users": users, "torrents": torrents, "banned_users": banned,
        "operator": auth.id,
    })))
}

//! GET /admin/settings/history：单项修改历史（audit_log）。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

// GET /admin/settings/history —— 单项修改历史（audit_log）
// ============================================================================

#[derive(Deserialize)]
struct HistoryQ {
    name: String,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct HistoryRow {
    id: i64,
    action: String,
    actor: Option<String>,
    actor_id: Option<i64>,
    detail: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/settings/history")]
pub(super) async fn settings_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<HistoryQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let per_page = q.per_page.clamp(1, 100);
    let name = q.name.trim();
    let rows: Vec<HistoryRow> = sqlx::query_as(
        "SELECT a.id, a.action, u.username AS actor, a.actor_id, a.ref AS detail, a.created_at \
         FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id \
         WHERE a.action = 'setting:update' AND a.ref->>'setting' = $1 \
         ORDER BY a.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(name)
    .bind(per_page)
    .bind((q.page.max(1) - 1) * per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = \
         'setting:update' AND ref->>'setting' = $1",
    )
    .bind(name)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "name": name,
        "rows": rows,
        "total": total,
        "page": q.page.max(1),
        "per_page": per_page,
    })))
}

// ============================================================================

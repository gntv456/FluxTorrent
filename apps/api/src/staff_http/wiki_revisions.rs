//! Wiki 版本史读口（批五十五按域拆出，原在 `wiki.rs`）：
//! `GET /admin/wiki/{id}/revisions`。
//!
//! 拆件理由与 `wiki_public.rs` 同一档：写侧（CRUD + 事务存档）与读侧（历史只读）
//! 的判据不同，挤在一件里超了 300 行门禁，且「回滚不做」这类口径只属于读侧文件。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::wiki::require_wiki_manage;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// GET /admin/wiki/{id}/revisions：版本历史（照 v4_http rule_revisions，
/// LIMIT 50 新况在前；只读，回滚不做——最小落地）
#[get("/admin/wiki/{id}/revisions")]
pub async fn wiki_revisions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    let rows: Vec<WikiRevisionRow> = sqlx::query_as(
        "SELECT r.id, r.page_id, r.title, r.body, r.sort, \
         u.username AS edited_by_name, r.created_at \
         FROM wiki_revisions r LEFT JOIN users u ON u.id = r.edited_by \
         WHERE r.page_id = $1 ORDER BY r.created_at DESC LIMIT 50",
    )
    .bind(*path)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct WikiRevisionRow {
    id: i64,
    page_id: i64,
    title: String,
    body: String,
    sort: i32,
    edited_by_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

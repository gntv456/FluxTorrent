//! P2-6 改名记录 / 用户修改记录
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-6 改名记录 / 用户修改记录 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct RenameLogRow {
    id: i64,
    uid: i64,
    username: String,
    old_name: String,
    new_name: String,
    operator: Option<i64>,
    operator_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct UidPageQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/rename-logs")]
async fn admin_rename_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UidPageQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<RenameLogRow> = sqlx::query_as(
        r#"SELECT l.id, l.uid, u.username, l.old_name, l.new_name,
                  l.operator, op.username AS operator_name, l.created_at
           FROM username_change_logs l
           JOIN users u ON u.id = l.uid
           LEFT JOIN users op ON op.id = l.operator
           WHERE ($1::bigint IS NULL OR l.uid = $1)
           ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM username_change_logs WHERE ($1::bigint \
         IS NULL OR uid = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ModifyLogRow {
    id: i64,
    uid: i64,
    username: String,
    modifier: Option<i64>,
    modifier_name: Option<String>,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/modify-logs")]
async fn admin_modify_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UidPageQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<ModifyLogRow> = sqlx::query_as(
        r#"SELECT l.id, l.uid, u.username, l.modifier, op.username AS modifier_name,
                  l.content, l.created_at
           FROM user_modify_logs l
           JOIN users u ON u.id = l.uid
           LEFT JOIN users op ON op.id = l.modifier
           WHERE ($1::bigint IS NULL OR l.uid = $1)
           ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_modify_logs WHERE ($1::bigint IS \
         NULL OR uid = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

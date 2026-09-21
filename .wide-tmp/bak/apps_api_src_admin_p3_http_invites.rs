//! P2-4 邀请管理（参考站 user/invites 口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-4 邀请管理（参考站 user/invites 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct InviteAdminRow {
    id: i64,
    inviter: String,
    inviter_id: i64,
    code: String,
    status: i16,
    used_by: Option<i64>,
    used_by_name: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct InviteListQ {
    #[serde(default)]
    uid: Option<i64>,
    /// 0未用 1已用 2过期 3撤销；缺省全部
    #[serde(default)]
    valid: Option<i16>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/invites")]
async fn admin_invites(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<InviteListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::INVITE_VIEW)
        .await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<InviteAdminRow> = sqlx::query_as(
        r#"SELECT i.id, u.username AS inviter, i.inviter_id, i.code, i.status,
                  i.used_by, uu.username AS used_by_name, i.expires_at
           FROM invites i
           JOIN users u ON u.id = i.inviter_id
           LEFT JOIN users uu ON uu.id = i.used_by
           WHERE ($1::bigint IS NULL OR i.inviter_id = $1)
             AND ($2::smallint IS NULL OR i.status = $2)
           ORDER BY i.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.valid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invites i WHERE ($1::bigint IS NULL OR i.inviter_id = $1) \
         AND ($2::smallint IS NULL OR i.status = $2)",
    )
    .bind(q.uid)
    .bind(q.valid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

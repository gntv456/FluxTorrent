//! P2-4 邀请管理（参考站 user/invites 口径）
//! 从 admin_p3_http.rs 按域拆出。0204：发码/作废 + 邮箱/被邀请人筛选。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
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
    /// 发送对象邮箱（邮件邀请；0204 起管理端可见）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
struct InviteListQ {
    #[serde(default)]
    uid: Option<i64>,
    /// 0未用 1已用 2过期 3撤销；缺省全部
    #[serde(default)]
    valid: Option<i16>,
    /// 邮箱模糊筛选（邮件邀请场景，0204）
    #[serde(default)]
    email: Option<String>,
    /// 被邀请人用户名精确筛选（找「谁用了某人的码」，0204）
    #[serde(default)]
    used_by_name: Option<String>,
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
                  i.used_by, uu.username AS used_by_name, i.expires_at, i.email
           FROM invites i
           JOIN users u ON u.id = i.inviter_id
           LEFT JOIN users uu ON uu.id = i.used_by
           WHERE ($1::bigint IS NULL OR i.inviter_id = $1)
             AND ($2::smallint IS NULL OR i.status = $2)
             AND ($3::text IS NULL OR i.email ILIKE '%' || $3 || '%')
             AND ($4::text IS NULL OR uu.username = $4)
           ORDER BY i.id DESC LIMIT $5 OFFSET $6"#,
    )
    .bind(q.uid)
    .bind(q.valid)
    .bind(q.email.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(
        q.used_by_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM invites i
           LEFT JOIN users uu ON uu.id = i.used_by
           WHERE ($1::bigint IS NULL OR i.inviter_id = $1)
             AND ($2::smallint IS NULL OR i.status = $2)
             AND ($3::text IS NULL OR i.email ILIKE '%' || $3 || '%')
             AND ($4::text IS NULL OR uu.username = $4)"#,
    )
    .bind(q.uid)
    .bind(q.valid)
    .bind(q.email.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(
        q.used_by_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

/// 管理端直发邀请码（0204 P1）：给指定用户发 N 天码（上限 50）。
/// NP takeinvite 口径；天数缺省读 invite_admin_ttl_days（30）。
#[derive(Deserialize)]
struct AdminInviteIssueReq {
    user_id: i64,
    #[serde(default = "one")]
    count: i64,
    #[serde(default)]
    days: Option<i64>,
}

fn one() -> i64 {
    1
}

#[post("/admin/invites")]
async fn admin_invite_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdminInviteIssueReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    if !(1..=50).contains(&body.count) {
        return Err(DomainError::Validation("count 取值 1-50".into()));
    }
    // 天数：显式传入优先；缺省读 invite_admin_ttl_days（30）
    let days = match body.days {
        Some(d) => d.clamp(1, 365),
        None => crate::economy_http::invite_admin_ttl_days(&state.repo.db)
            .await,
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status < 2)",
    )
    .bind(body.user_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(body.user_id));
    }
    let mut codes = Vec::new();
    for _ in 0..body.count {
        let code = crate::domain::new_invite_code();
        let expires =
            chrono::Utc::now() + chrono::Duration::days(days);
        let id = state.repo.issue_invite(body.user_id, &code, expires).await?;
        codes.push(serde_json::json!({
            "id": id, "code": code, "expires_at": expires.to_rfc3339(),
        }));
    }
    state
        .repo
        .audit(Some(auth.id), "admin.invite_issue", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id, "days": days, "codes": codes,
    })))
}

/// 管理端作废邀请码（0204 P1）：任意未用码 status 0→3（撤销留痕，
/// 与用户侧 DELETE /invites/{id} 同状态位；区别在于无需属主校验）。
#[derive(Deserialize)]
struct AdminInviteRevokeReq {
    id: i64,
}

#[post("/admin/invites/revoke")]
async fn admin_invite_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdminInviteRevokeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    let n = sqlx::query(
        "UPDATE invites SET status = 3 WHERE id = $1 AND status = 0",
    )
    .bind(body.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "邀请码不存在、已使用或已撤销".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "admin.invite_revoke", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "revoked": body.id })))
}

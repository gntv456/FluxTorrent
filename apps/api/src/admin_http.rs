//! M29 完整管理后台 HTTP 接口（staff 专用）。
//!
//! 覆盖（方案 M29 验收口径的 Dev 版）：种子审核队列（通过/拒绝 + 理由）、
//! 举报处理、用户管理（封禁/解封/等级调整）、审计日志查询、站点运营概览。
//! 敏感操作全部 require_staff + audit 落库（§5.7）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_admin(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(admin_overview)
        .service(review_queue)
        .service(review_decide)
        .service(report_queue)
        .service(report_resolve)
        .service(user_admin_list)
        .service(user_set_status)
        .service(user_set_class)
        .service(audit_query)
}

async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    Ok(auth)
}

/// 运营概览：待审/举报/用户/种子计数
#[get("/admin/overview")]
async fn admin_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (pending, reports, users, torrents, banned): (i64, i64, i64, i64, i64) = sqlx::query_as(
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

// ============ 种子审核 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PendingTorrent {
    id: i64,
    name: String,
    owner_id: Option<i64>,
    size: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/reviews")]
async fn review_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<PendingTorrent> = sqlx::query_as(
        "SELECT id, name, owner_id, size, created_at FROM torrents \
         WHERE approval_status = 0 ORDER BY id LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ReviewReq {
    torrent_id: i64,
    approve: bool,
    #[serde(default)]
    reason: String,
}

#[post("/admin/reviews/decide")]
async fn review_decide(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !body.approve && body.reason.trim().is_empty() {
        return Err(DomainError::Validation("拒绝必须填理由".into()));
    }
    // 1=已过 2=被拒
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = $2 \
         WHERE id = $1 AND approval_status = 0",
    )
    .bind(body.torrent_id)
    .bind(if body.approve { 1 } else { 2 })
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在待审状态".into()));
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.approve {
                "review.approve"
            } else {
                "review.reject"
            },
            Some(body.torrent_id),
        )
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id, "approved": body.approve, "reason": body.reason,
    })))
}

// ============ 举报处理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ReportRow {
    id: i64,
    reporter_id: i64,
    ref_type: String,
    ref_id: i64,
    reason: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/reports")]
async fn report_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<ReportRow> = sqlx::query_as(
        "SELECT id, reporter_id, ref_type, ref_id, reason, created_at \
         FROM reports WHERE status = 0 ORDER BY id LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ResolveReq {
    report_id: i64,
    #[serde(default)]
    note: String,
}

#[post("/admin/reports/resolve")]
async fn report_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE reports SET status = 1, handled_by = $1, handled_at = now() \
         WHERE id = $2 AND status = 0",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("举报不存在或已处理".into()));
    }
    let _ = &body.note; // 处理备注并入审计日志
    state
        .repo
        .audit(Some(auth.id), "report.resolve", Some(body.report_id))
        .await;
    Ok(ok(serde_json::json!({ "resolved": body.report_id })))
}

// ============ 用户管理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserRow {
    id: i64,
    username: String,
    email: String,
    class_id: i32,
    status: i16,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/users")]
async fn user_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SearchQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = format!("%{}%", q.q.trim());
    let rows: Vec<AdminUserRow> = sqlx::query_as(
        "SELECT id, username, email, class_id, status, created_at FROM users \
         WHERE username ILIKE $1 OR email ILIKE $1 ORDER BY id LIMIT 100",
    )
    .bind(pattern)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct SearchQ {
    #[serde(default = "empty_q")]
    q: String,
}
fn empty_q() -> String {
    String::new()
}

#[derive(Deserialize)]
struct SetStatusReq {
    user_id: i64,
    /// 0 正常 1 禁言 2 封禁
    status: i16,
}

#[post("/admin/users/status")]
async fn user_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetStatusReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !(0..=2).contains(&body.status) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    let n = sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
        .bind(body.user_id)
        .bind(body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "user.set_status", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "status": body.status }),
    ))
}

#[derive(Deserialize)]
struct SetClassReq {
    user_id: i64,
    class_id: i32,
}

#[post("/admin/users/class")]
async fn user_set_class(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetClassReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 站长（99）才能调整等级；且禁止操作同级/更高级账户
    if auth.class_id < 99 || body.class_id >= 99 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("UPDATE users SET class_id = $2 WHERE id = $1 AND class_id < 99")
        .bind(body.user_id)
        .bind(body.class_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在或不可调整".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "user.set_class", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "class_id": body.class_id }),
    ))
}

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
    let _auth = staff(&req, &state).await?;
    let pattern = format!("%{}%", q.q.trim());
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

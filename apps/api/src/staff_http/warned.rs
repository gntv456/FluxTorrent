//! 警告处理与 IP 工具。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/warned")]
pub async fn warned_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    let rows: Vec<WarnedRow> = sqlx::query_as(
        "SELECT id, username, warned_until, warned_reason FROM users WHERE warned_until > now() ORDER BY warned_until",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WarnBody {
    user_id: i64,
    weeks: i32,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/warned")]
pub async fn warn_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WarnBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    if !(1..=52).contains(&body.weeks) {
        return Err(DomainError::Validation("警告时长需 1-52 周".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET warned_until = now() + make_interval(weeks => $2), warned_reason = $3 WHERE id = $1 AND status < 2",
    ).bind(body.user_id).bind(body.weeks).bind(&body.reason)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "warn_user", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "warned": body.user_id, "until_weeks": body.weeks }),
    ))
}

#[delete("/admin/warned/{user_id}")]
pub async fn unwarn_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    let n = sqlx::query("UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state
        .repo
        .audit(Some(auth.id), "unwarn_user", Some(*path))
        .await;
    Ok(ok(serde_json::json!({ "unwarned": *path })))
}

/// 警告用户列表（warned.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct WarnedRow {
    pub(super) id: i64,
    pub(super) username: String,
    pub(super) warned_until: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) warned_reason: Option<String>,
}

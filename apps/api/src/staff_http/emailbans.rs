//! 邮件黑名单与 testip。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/emailbans")]
pub async fn emailban_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    let rows: Vec<EmailBanRow> = sqlx::query_as(
        "SELECT e.id, e.pattern, e.mode, e.note, u.username AS created_by, e.created_at \
         FROM email_bans e LEFT JOIN users u ON u.id = e.created_by ORDER BY e.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct EmailBanBody {
    pattern: String,
    mode: String, // ban | allow
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/emailbans")]
pub async fn emailban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EmailBanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    if !body.pattern.contains('@')
        && !body.pattern.starts_with('@')
        && !body.pattern.ends_with('@')
    {
        return Err(DomainError::Validation(
            "格式需为邮箱、@domain 或 user@ 通配".into(),
        ));
    }
    if !["ban", "allow"].contains(&body.mode.as_str()) {
        return Err(DomainError::Validation("mode 需为 ban 或 allow".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO email_bans (pattern, mode, note, created_by) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (pattern) DO UPDATE SET mode = EXCLUDED.mode, note = EXCLUDED.note RETURNING id",
    ).bind(body.pattern.trim()).bind(&body.mode).bind(&body.note).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emailban.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/emailbans/{id}")]
pub async fn emailban_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    sqlx::query("DELETE FROM email_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emailban.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// IP 测试（testip.php 口径）：检测 IP 是否命中封禁列表
#[derive(Deserialize)]
struct TestIpQuery {
    ip: String,
}

#[get("/admin/testip")]
pub async fn test_ip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TestIpQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TESTIP)
        .await?;
    let ip: std::net::IpAddr =
        q.ip.trim()
            .parse()
            .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let hit: Option<(String, Option<String>, String)> = sqlx::query_as(
                "SELECT host(ip), reason, \
         COALESCE(u.username, 'system') FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by WHERE ip = $1::inet",
    ).bind(&ip_text)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 该 IP 最近登录的账号
    let users: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT u.username FROM login_events le JOIN users u ON u.id = le.user_id \
         WHERE le.ip = $1::inet AND le.user_id > 0 LIMIT 10",
    )
    .bind(&ip_text)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "ip": ip_text,
        "banned": hit.is_some(),
        "reason": hit.as_ref().map(|h| h.1.clone()).flatten(),
        "by": hit.as_ref().map(|h| h.2.clone()),
        "seen_users": users,
    })))
}

/// 邮箱黑白名单（bannedemails/allowedemails.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct EmailBanRow {
    pub(super) id: i32,
    pub(super) pattern: String,
    pub(super) mode: String,
    pub(super) note: Option<String>,
    pub(super) created_by: Option<String>,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

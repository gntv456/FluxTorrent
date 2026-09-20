//! 封禁管理（bans）。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{bump_guard_ver, require_auth};
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/bans")]
pub async fn ban_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let rows: Vec<IpBanRow> = sqlx::query_as(
        "SELECT b.id, b.ip::text AS ip, b.reason, u.username AS banned_by, b.created_at \
         FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by ORDER BY b.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BanBody {
    ip: String,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/bans")]
pub async fn ban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let ip: std::net::IpAddr = body
        .ip
        .trim()
        .parse()
        .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ip_bans (ip, reason, banned_by) VALUES ($1::inet, $2, $3) ON CONFLICT (ip) DO UPDATE SET reason = EXCLUDED.reason RETURNING id",
    ).bind(ip_text).bind(&body.reason).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_ban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/bans/{id}")]
pub async fn ban_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    sqlx::query("DELETE FROM ip_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_unban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- staffpanel 运营工具：种子促销 / 批量私信 / 添加用户 / 增加魔力 / 警告用户 / 重复IP / 失败登录 ----

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct IpBanRow {
    pub(super) id: i32,
    pub(super) ip: String,
    pub(super) reason: Option<String>,
    pub(super) banned_by: Option<String>,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

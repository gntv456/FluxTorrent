//! P2-10 按 IP 解封（登录记录页一键封/解封配套）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-10 按 IP 解封（登录记录页一键封/解封配套） ============

#[derive(Deserialize)]
struct BanByIpReq {
    ip: String,
}

#[post("/admin/bans/by-ip/delete")]
async fn admin_ban_delete_by_ip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanByIpReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let ip = body.ip.trim();
    if ip.is_empty() {
        return Err(DomainError::Validation("IP 不能为空".into()));
    }
    let n = sqlx::query("DELETE FROM ip_bans WHERE ip = $1::inet")
        .bind(ip)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    state.repo.audit(Some(auth.id), "ban.del_by_ip", None).await;
    // 解封需立即作用于请求入口与 tracker（ip_bans 强制校验 + 防护缓存）
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

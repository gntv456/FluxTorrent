//! 认证字幕人管理端（0149）：手动授予/撤销 + 持有者列表。
//! 自动授予在 worker（jobs/subtitle_cert.rs）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

#[derive(Deserialize)]
struct CertGrantReq {
    user_id: i64,
    /// certified / gold
    tier: String,
    #[serde(default)]
    reason: String,
}

/// 手动授予（staff）：source=admin 的行不被自动降级
#[post("/admin/subtitles/certs/grant")]
async fn certs_grant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CertGrantReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !["certified", "gold"].contains(&body.tier.as_str()) {
        return Err(DomainError::Validation("tier 需为 certified/gold".into()));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
            .bind(body.user_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(body.user_id));
    }
    sqlx::query(
        "INSERT INTO user_subtitle_certs (user_id, tier, source, reason, \
         granted_by) VALUES ($1, $2, 'admin', $3, $4) ON CONFLICT (user_id, \
         tier) DO UPDATE SET revoked_at = NULL, reason = $3, granted_by = $4",
    )
    .bind(body.user_id)
    .bind(&body.tier)
    .bind(body.reason.trim())
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "subcert.grant", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id, "tier": body.tier,
    })))
}

/// 撤销（staff；可撤 auto 行 = 关掉某人自动档）
#[post("/admin/subtitles/certs/revoke")]
async fn certs_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CertGrantReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE user_subtitle_certs SET revoked_at = now() WHERE user_id = \
         $1 AND tier = $2 AND revoked_at IS NULL",
    )
    .bind(body.user_id)
    .bind(&body.tier)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该用户没有此身份或已撤销".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "subcert.revoke", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id, "tier": body.tier, "revoked": true,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct CertRow {
    user_id: i64,
    username: Option<String>,
    tier: String,
    source: String,
    reason: String,
    granted_at: chrono::DateTime<chrono::Utc>,
}

/// 持有者列表（在榜身份，含 gold/certified 分组）
#[get("/admin/subtitles/certs")]
async fn certs_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<CertRow> = sqlx::query_as(
        "SELECT c.user_id, u.username, c.tier, c.source, c.reason, \
         c.granted_at FROM user_subtitle_certs c LEFT JOIN users u ON u.id \
         = c.user_id WHERE c.revoked_at IS NULL ORDER BY c.tier, \
         c.granted_at DESC LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

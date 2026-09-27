//! Passkey 端点汇总挂载：remove + 三仪式组合（300 行门禁）。
//! 工具在 passkey_util.rs、注册仪式 passkey_reg.rs、登录仪式 passkey_login.rs。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};

use super::passkey_login::{passkey_login_begin, passkey_login_finish};
use super::passkey_reg::{passkey_begin, passkey_finish};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 我的凭证列表（登录态；PasskeysCard 消费）
#[get("/me/passkeys")]
async fn passkey_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    type PasskeyRow = (
        String,
        String,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
    );
    let rows: Vec<PasskeyRow> = sqlx::query_as(
        "SELECT label, cred_id, created_at, last_used_at \\
         FROM user_passkeys WHERE user_id = $1 ORDER BY id DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "items": rows.iter().map(|(l, c, cr, lu)| serde_json::json!({
            "label": l, "cred_id": c, "created_at": cr, "last_used_at": lu,
        })).collect::<Vec<_>>(),
    })))
}

/// 我的凭证列表 + 解绑（登录态）
#[post("/auth/passkeys/remove")]
async fn passkey_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<serde_json::Value>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let cred = body
        .get("cred_id")
        .and_then(|v| v.as_str())
        .ok_or(DomainError::Validation("缺少 cred_id".into()))?;
    sqlx::query(
        "DELETE FROM user_passkeys WHERE user_id = $1 AND cred_id = $2",
    )
    .bind(auth.id)
    .bind(cred)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "passkey_remove", None)
        .await;
    Ok(ok(serde_json::json!({ "removed": true })))
}

pub fn mount_passkey(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(passkey_list)
        .service(passkey_begin)
        .service(passkey_finish)
        .service(passkey_login_begin)
        .service(passkey_login_finish)
        .service(passkey_remove)
}

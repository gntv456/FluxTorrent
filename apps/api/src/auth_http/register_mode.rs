//! GET /register-mode：注册模式公开下发（0204，0227 扩展）。
//! open 模式下前端不强制邀请码；0227 附带验证码驱动配置与申请通道开关。

use actix_web::{get, web, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// GET /register-mode：注册模式公开下发（0204）——open 模式下前端
/// 不再强制邀请码（与后端 registration_mode 判定同步）。
#[get("/register-mode")]
pub async fn register_mode_endpoint(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    async fn setting(db: &sqlx::PgPool, name: &str, dflt: &str) -> String {
        sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \\
             WHERE name = $1), $2)",
        )
        .bind(name)
        .bind(dflt)
        .fetch_one(db)
        .await
        .unwrap_or_else(|_| dflt.to_string())
    }
    let mode =
        setting(&state.repo.db, "registration_mode", "invite_only").await;
    let provider = setting(&state.repo.db, "captcha_provider", "none").await;
    let site_key = setting(&state.repo.db, "captcha_site_key", "").await;
    let app_open = setting(&state.repo.db, "application_signup", "off").await;
    Ok(ok(serde_json::json!({
        "mode": mode,
        "captcha_provider": provider,
        "captcha_site_key": site_key,
        "application_signup": app_open,
    })))
}

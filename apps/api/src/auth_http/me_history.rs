//! me 登录历史（M01）：GET /me/logins。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/me/logins")]
pub async fn my_login_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<LoginEventRow> = sqlx::query_as(
        "SELECT created_at, host(ip) AS ip, ok, user_agent, reason          FROM login_events WHERE user_id = $1 ORDER BY id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LoginEventRow {
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    ip: Option<String>,
    ok: bool,
    #[sqlx(default)]
    user_agent: String,
    /// 0=成功 1=密码错误 3=停用账号 4=未知用户名
    #[sqlx(default)]
    reason: i16,
}

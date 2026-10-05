//! 会话管理（0284 P1-3）：GET /me/sessions 活跃设备近似 +
//! POST /me/sessions/revoke-others 踢出其它会话。
//!
//! 「活跃会话」以 login_events 近 24h 成功登录按 (ip, ua) 分组近似——
//! JWT 无会话存储，无法精确枚举在途 token；踢出其它会话用撤销线
//! nbf=now 实现（本请求所持 token 的 iat == now 边界，因 iat<=nbf 判死
//! 的口径恰好保住自己）。代价：其它设备上更早签发的 token 立即失效。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct SessionRow {
    ip: String,
    user_agent: String,
    last_at: chrono::DateTime<chrono::Utc>,
    /// 同 (ip, ua) 的成功登录次数
    logins: i64,
}

#[get("/me/sessions")]
pub async fn my_sessions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<SessionRow> = sqlx::query_as(
        "SELECT ip, user_agent, max(created_at) AS last_at, \
             count(*)::bigint AS logins \
         FROM login_events WHERE user_id = $1 AND ok AND \
         created_at > now() - interval '24 hours' \
         GROUP BY ip, user_agent ORDER BY last_at DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/me/sessions/revoke-others")]
pub async fn revoke_other_sessions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // nbf = 本 token 的 iat - 1（改密路径同款口径，auth_infra 用 iat <= nbf 判死）：
    // 更早签发的其它设备凭证全部失效；本 token（iat == nbf+1）恰好活下来。
    // 代价：与本次请求同一秒内签发的其它设备 token 也会幸存——可接受。
    sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET nbf = \
         GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(auth.iat - 1)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let _: Result<(), _> =
        redis::AsyncCommands::set_ex(&mut c, &key, auth.iat - 1, 86400u64)
            .await;
    Ok(ok(serde_json::json!({ "revoked": "others" })))
}

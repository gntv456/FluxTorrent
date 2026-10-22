//! 会话管理（0284 P1-3）：GET /me/sessions 活跃设备近似 +
//! POST /me/sessions/revoke-others 踢出其它会话。
//!
//! 「活跃会话」以 login_events 近 24h 成功登录按 (ip, ua) 分组近似——
//! JWT 无会话存储，无法精确枚举在途 token；踢出其它会话用撤销线
//! nbf=now-1 实现（P-564：旧口径 nbf=本 token iat-1 漏掉「比本设备更晚
//! 登录」的会话），并给本设备重签一枚新 token 随响应写回 cookie。
//! 代价：同秒签发的其它 token 仍会幸存（iat 只有秒级）。

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
        // host(ip)：login_events.ip 是 inet，直接投影给 String 会被 sqlx 判
        // 类型不合（mismatched types … INET），「活跃会话」卡整卡 500。
        // 同表其余读口（me_history/admin records/ipcheck）一贯用 host()。
        "SELECT host(ip) AS ip, user_agent, max(created_at) AS last_at, \
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
    // P-564：旧写法 nbf=auth.iat-1 只能判死「比本 token 更早签发」的会话，
    // 比本设备更晚登录的（攻击者登录在你之后）反而幸存。
    // 改为 now-1：所有既有凭证失效，再给本设备重签一枚（iat=now > nbf）
    // 随响应写回 cookie——文案「其它所有设备失效、本设备不受影响」才成立。
    // 同秒签发的其它 token 仍会幸存（iat 只有秒级），与旧口径取舍一致。
    let nbf = chrono::Utc::now().timestamp() - 1;
    let ttl_hours: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'jwt_ttl_hours')::bigint, 24)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(24)
    .clamp(1, 720);
    let token = state
        .jwt
        .issue(auth.id, auth.class_id, ttl_hours)
        .map_err(DomainError::Internal)?;
    sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET nbf = \
         GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(nbf)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let _: Result<(), _> =
        redis::AsyncCommands::set_ex(&mut c, &key, nbf, 86400u64).await;
    let mut resp = ok(serde_json::json!({
        "revoked": "others",
        "token": token
    }));
    let cookie = actix_web::cookie::Cookie::build("flux_token", token)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::hours(ttl_hours))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(
            req.headers()
                .get("x-forwarded-proto")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("https"))
                .unwrap_or(false),
        )
        .finish()
        .to_string();
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&cookie)
            .expect("cookie 串解析为 HeaderValue 必然成功"),
    );
    Ok(resp)
}

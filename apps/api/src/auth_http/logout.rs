//! 登出（M01）：POST /auth/logout（清 cookie + 失效会话）。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

#[post("/auth/logout")]
pub async fn logout(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 令牌撤销（§5.7）：nbf = 本次凭证 iat —— require_auth 用 iat<=nbf 判死，
    // 因此登出所用的 token 与一切更早签发的立即失效；登出后新登录（iat 严格更大）不受影响。
    // 撤销线 DB 权威（0085）+ Redis 加速缓存
    // 审计修复（P2 吞错）：撤销是安全语义操作，双写（DB 权威 + Redis 加速）失败必须
    // 留痕——旧版全部静默吞掉，双双失败时 token 仍有效且无任何日志可查。
    if let Err(e) = sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2)          ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(auth.iat)
    .execute(&state.repo.db)
    .await
    {
        tracing::error!(user_id = auth.id, error = ?e, "登出撤销线 DB 写入失败：token 在 24h 内仍可能通过校验");
    }
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let redis_res: Result<(), _> =
        redis::AsyncCommands::set_ex(&mut c, &key, auth.iat, 86400u64).await;
    if let Err(e) = redis_res {
        tracing::warn!(user_id = auth.id, error = ?e, "登出撤销线 Redis 写入失败（DB 权威仍在，影响为加速缓存缺失）");
    }
    state.repo.audit(Some(auth.id), "auth.logout", None).await;
    // 同步清除 HttpOnly 会话 cookie（与撤销线配合：即便 token 被复用，cookie 已不存在）
    let clear = actix_web::cookie::Cookie::build("flux_token", "")
        .path("/")
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .finish()
        .to_string();
    let mut resp = ok(serde_json::json!({ "ok": true }));
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&clear).expect("清 cookie 串解析必然成功"),
    );
    Ok(resp)
}

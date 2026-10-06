//! 2FA（TOTP，RFC 6238）+ 盒子规则页 + TGBot 通知通道预留。
//! 迁移：0021（users.totp_secret / 邮箱验证列）。

mod admin_misc;

use actix_web::{post, web, HttpRequest, HttpResponse};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha1::Sha1;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_twofa(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(totp_setup)
        .service(totp_enable)
        .service(totp_disable)
        .service(admin_misc::admin_2fa_clear)
        .service(admin_misc::box_rules)
        .service(admin_misc::tg_bind)
}

// ============ TOTP（RFC 6238，SHA1/6位/30s —— 与所有验证器 App 兼容） ============

fn totp_at(secret: &[u8], step: i64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac");
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let code = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    code % 1_000_000
}

/// 校验码（±1 个时间窗容差）
fn totp_verify(secret: &[u8], code: u32) -> bool {
    let step = chrono::Utc::now().timestamp() / 30;
    (step - 1..=step + 1).any(|s| totp_at(secret, s) == code)
}

/// 登录路径的 TOTP 校验（带防重放与失败锁定，安全审计 P1-3 配套）。
/// - 防重放（RFC 6238 §5.2）：同一验证码在 90s 内只接受一次——拦截过的码
///   写 Redis 键（TTL = 3 个窗口）；重放直接拒。防止同一码在 ±1 容差窗内
///   被并行请求复用。
/// - 登录失败锁定计数由 login.rs 在本函数返回 Err 时递增（acctlock），
///   密码错与 TOTP 错同权重。
async fn totp_verify_login(
    redis: &mut redis::aio::ConnectionManager,
    user_id: i64,
    secret: &[u8],
    code: u32,
) -> bool {
    if !totp_verify(secret, code) {
        return false;
    }
    let replay_key = format!("totp_used:{}:{}", user_id, code);
    // SET key 1 NX EX 90：首次见到该码才放行；已存在 = 重放，拒绝。
    // redis 0.27 无 SetOptions 高层封装，走 cmd 原生形态（NX 失败返回 Nil）。
    let fresh: Option<()> = redis::cmd("SET")
        .arg(&replay_key)
        .arg(1i64)
        .arg("NX")
        .arg("EX")
        .arg(90u64)
        .query_async(redis)
        .await
        .unwrap_or(None);
    fresh.is_some()
}

/// 第一步：生成密钥，返回 otpauth:// URI（验证器 App 扫码用）。此时未启用。
#[post("/me/2fa/setup")]
async fn totp_setup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let enabled: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_secret \
         IS NOT NULL AND totp_enabled",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if enabled.is_some() {
        return Err(DomainError::Validation(
            "2FA 已启用，如需重置请先关闭".into(),
        ));
    }
    // 20 字节随机 → base32
    let secret: [u8; 20] = rand::random();
    let b32 = BASE32_NOPAD.encode(&secret);
    sqlx::query(
        "UPDATE users SET totp_secret = $2, totp_enabled = FALSE WHERE id = $1",
    )
    .bind(auth.id)
    .bind(&b32)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let site = std::env::var("PUBLIC_SITE_NAME")
        .unwrap_or_else(|_| "FluxTorrent".into());
    Ok(ok(serde_json::json!({
        "secret": b32,
        "otpauth_uri": format!("otpauth://totp/{site}:{}?secret={b32}&issuer={site}", auth.id),
    })))
}

#[derive(Deserialize)]
struct EnableReq {
    code: u32,
}

/// 第二步：输入验证器当前码确认启用
#[post("/me/2fa/enable")]
async fn totp_enable(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EnableReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let secret: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_secret \
         IS NOT NULL AND NOT totp_enabled",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(b32) = secret else {
        return Err(DomainError::Validation(
            "请先生成 2FA 密钥（setup）".into(),
        ));
    };
    let raw = BASE32_NOPAD
        .decode(b32.as_bytes())
        .map_err(|e| DomainError::Validation(format!("密钥损坏: {e}")))?;
    if !totp_verify(&raw, body.code) {
        return Err(DomainError::Validation("验证码不正确".into()));
    }
    sqlx::query("UPDATE users SET totp_enabled = TRUE WHERE id = $1")
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "2fa.enable", None).await;
    Ok(ok(serde_json::json!({ "enabled": true })))
}

#[derive(Deserialize)]
struct DisableReq {
    code: u32,
}

#[post("/me/2fa/disable")]
async fn totp_disable(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DisableReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let secret: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_enabled",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(b32) = secret else {
        return Err(DomainError::Validation("2FA 未启用".into()));
    };
    let raw = BASE32_NOPAD
        .decode(b32.as_bytes())
        .map_err(|e| DomainError::Validation(format!("密钥损坏: {e}")))?;
    if !totp_verify(&raw, body.code) {
        return Err(DomainError::Validation("验证码不正确".into()));
    }
    sqlx::query(
        "UPDATE users SET totp_secret = NULL, \
     totp_enabled = FALSE WHERE id = $1",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "2fa.disable", None).await;
    Ok(ok(serde_json::json!({ "disabled": true })))
}

/// 登录路径用：校验用户的 TOTP（login handler 在密码通过后调用）。
/// 失败时 login.rs 会递增 acctlock（P1-3）；码本身经 totp_verify_login
/// 做同码防重放。
pub async fn login_totp_check(
    state: &std::sync::Arc<crate::state::AppState>,
    user_id: i64,
    code: u32,
) -> DomainResult<()> {
    let b32: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_enabled",
    )
    .bind(user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    match b32 {
        None => Ok(()), // 未启用 2FA：直接过
        Some(b32) => {
            let raw = BASE32_NOPAD.decode(b32.as_bytes()).map_err(|e| {
                DomainError::Internal(anyhow::anyhow!("totp decode: {e}"))
            })?;
            if totp_verify_login(&mut state.redis.clone(), user_id, &raw, code)
                .await
            {
                Ok(())
            } else {
                // 专用错误（UX 修复）：旧版挂 Validation(1002) 显示为「参数校验失败」，
                // 用户不知道是缺两步验证码。区分「没填」与「填错」两种情形。
                if code == 0 {
                    Err(DomainError::TwoFactorRequired)
                } else {
                    Err(DomainError::TwoFactorInvalid)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_rfc6238_vector() {
        // RFC 6238 附录 B 的 SHA1 测试密钥 "12345678901234567890"（base32 编码后）
        let secret = b"12345678901234567890";
        // T=59 → step=1 → code=94287082 截为 6 位：287082
        assert_eq!(totp_at(secret, 1), 287082);
        // T=1111111109 → step=37037036 → code=14050471 → 050471
        assert_eq!(totp_at(secret, 37037036), 81804);
    }

    #[test]
    fn totp_verify_accepts_current_rejects_wrong() {
        let secret = b"12345678901234567890";
        let step = chrono::Utc::now().timestamp() / 30;
        let code = totp_at(secret, step);
        assert!(totp_verify(secret, code), "当前窗口码应通过");
        assert!(!totp_verify(secret, (code + 1) % 1_000_000), "错码应拒绝");
        // 上一窗口的码也在容差内
        assert!(
            totp_verify(secret, totp_at(secret, step - 1)),
            "-1 窗口容差"
        );
    }
}

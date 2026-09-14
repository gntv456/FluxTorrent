//! 2FA（TOTP，RFC 6238）+ 盒子规则页 + TGBot 通知通道预留。
//! 迁移：0021（users.totp_secret / 邮箱验证列）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
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
        .service(admin_2fa_clear)
        .service(box_rules)
        .service(tg_bind)
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

/// 第一步：生成密钥，返回 otpauth:// URI（验证器 App 扫码用）。此时未启用。
#[post("/me/2fa/setup")]
async fn totp_setup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let enabled: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_secret IS NOT NULL AND totp_enabled",
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
    sqlx::query("UPDATE users SET totp_secret = $2, totp_enabled = FALSE WHERE id = $1")
        .bind(auth.id)
        .bind(&b32)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let site = std::env::var("PUBLIC_SITE_NAME").unwrap_or_else(|_| "FluxTorrent".into());
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
        "SELECT totp_secret FROM users WHERE id = $1 AND totp_secret IS NOT NULL AND NOT totp_enabled",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(b32) = secret else {
        return Err(DomainError::Validation("请先生成 2FA 密钥（setup）".into()));
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
    let secret: Option<String> =
        sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = $1 AND totp_enabled")
            .bind(auth.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(b32) = secret else {
        return Err(DomainError::Validation("2FA 未启用".into()));
    };
    let raw = BASE32_NOPAD
        .decode(b32.as_bytes())
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !totp_verify(&raw, body.code) {
        return Err(DomainError::Validation("验证码不正确".into()));
    }
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled = FALSE WHERE id = $1")
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "2fa.disable", None).await;
    Ok(ok(serde_json::json!({ "disabled": true })))
}

/// 管理员清除用户 2FA（丢失验证器救援通道）。
/// 场景：用户丢失验证器 App 且无恢复码 → 账号被 login_totp_check 永久锁死。
/// 权限：user.resetpass（与重置密码同档：身份核验后由管理员人工放行）+ ensure_outranks。
#[post("/admin/users/{id}/2fa/clear")]
async fn admin_2fa_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_RESETPASS).await?;
    let uid = path.into_inner();
    // 等级护栏（与 admin_http 同口径）：操作者须严格高于目标用户
    {
        let db = &state.repo.db;
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let tc = target_class.ok_or(DomainError::NotFound(uid))?;
        if auth.class_id <= tc {
            return Err(DomainError::Forbidden);
        }
    }
    let updated = sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_enabled = FALSE \
         WHERE id = $1 AND (totp_enabled OR totp_secret IS NOT NULL)",
    )
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation("该用户未开启 2FA".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "2fa.admin_clear", Some(uid))
        .await;
    // 通知用户：2FA 已被管理员清除，下次登录仅需密码（建议尽快重新开启）
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(uid)
    .bind("两步验证已被管理员重置")
    .bind("你因丢失验证器申请救援，管理员已清除你的两步验证。下次登录仅需密码，请尽快在控制面板重新开启。")
    .execute(&state.repo.db)
    .await;
    Ok(ok(serde_json::json!({ "cleared": true, "user_id": uid })))
}

/// 登录路径用：校验用户的 TOTP（login handler 在密码通过后调用）
pub async fn login_totp_check(db: &sqlx::PgPool, user_id: i64, code: u32) -> DomainResult<()> {
    let b32: Option<String> =
        sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = $1 AND totp_enabled")
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    match b32 {
        None => Ok(()), // 未启用 2FA：直接过
        Some(b32) => {
            let raw = BASE32_NOPAD
                .decode(b32.as_bytes())
                .map_err(|e| DomainError::Internal(anyhow::anyhow!("totp decode: {e}")))?;
            if totp_verify(&raw, code) {
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

// ============ 盒子规则（声明页，NexusPHP box rules 口径） ============

#[get("/rules/box")]
async fn box_rules() -> impl Responder {
    ok(serde_json::json!({
        "title": "盒子 / 独服 / 高速下载设备规则",
        "updated": "2026-09-09",
        "rules": [
            "本站允许盒子做种，但禁止「只下不做」的刷流量行为；H&R 规则对盒子同样生效。",
            "禁止利用盒子进行恶意下载打击他人做种（hit & run 连发）。",
            "同 IP 并发做种 ≥ 50 个种子视为盒子行为，请在个人中心登记；未登记不影响功能，但申诉时以登记为准。",
            "使用云服务器做种请注意流量费用，本站不对超额流量负责。",
            "违反以上规则的账号按 H&R 追责流程处理，可通过申诉系统复核。"
        ],
    }))
}

// ============ TGBot 通知通道预留（绑定 → worker 推送位） ============

#[derive(Deserialize)]
struct TgBindReq {
    chat_id: String,
}

/// 绑定 Telegram（预留：worker 侧推送促销/短讯通知的投递目标）
#[post("/me/tg-bind")]
async fn tg_bind(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TgBindReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !body.chat_id.chars().all(|c| c.is_ascii_digit()) || body.chat_id.len() < 5 {
        return Err(DomainError::Validation("Telegram chat_id 应为数字".into()));
    }
    sqlx::query("UPDATE users SET tg_chat_id = $2 WHERE id = $1")
        .bind(auth.id)
        .bind(&body.chat_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "tg.bind", None).await;
    Ok(ok(serde_json::json!({ "bound": body.chat_id })))
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

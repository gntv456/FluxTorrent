//! me 安全面（M01）：passkey 轮换 + 改密码。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{bump_guard_ver, require_auth};
use crate::state::AppState;

#[post("/me/passkey/rotate")]
pub async fn rotate_passkey(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let pk = state.repo.update_passkey(auth.id).await?;
    state
        .repo
        .audit(Some(auth.id), "passkey_rotate", Some(auth.id))
        .await;
    // 旧 passkey 随 guard 版本轮询失效（tracker 侧 ≤3s，见 apps/tracker/src/main.rs
    // 的 flux:guard:ver 轮询清缓存）——不是字面「立即」，文案与注释同口径。
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "passkey": pk })))
}

/// 自助修改密码（usercp 口径）：验旧密码 → 换哈希 → 抬 logout_nbf 撤销既有 token
///（改密后所有设备下线，重新登录拿新 token —— 与 logout 同一套撤销机制）。

#[post("/me/password/change")]
pub async fn me_password_change(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasswordChangeReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.new_password.len() < 8 {
        return Err(DomainError::Validation("新密码至少 8 位".into()));
    }
    if body.new_password == body.old_password {
        return Err(DomainError::Validation("新密码不能与旧密码相同".into()));
    }
    let (pass_hash,): (String,) =
        sqlx::query_as("SELECT pass_hash FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if !domain::verify_password(&pass_hash, &body.old_password) {
        state
            .repo
            .audit(Some(auth.id), "password_change_fail", Some(auth.id))
            .await;
        return Err(DomainError::Validation("旧密码不正确".into()));
    }
    let new_hash = domain::hash_password(&body.new_password)?;
    // 改密同时轮换 passkey：引导期 root 等公开默认 passkey 不应在改密后继续可用。
    // 旧钥进 passkey_prev 宽限窗（审计 10-07 P1-4）：passkey 烤在用户已下载的
    // 每一个 .torrent 里，即刻失效等于手上所有种子集体停种。
    let new_passkey = uuid::Uuid::new_v4().simple().to_string();
    let rotated: Option<String> = sqlx::query_scalar(
        "UPDATE users SET pass_hash = $2, passkey = $3, \
            must_reset_password = false, \
            passkey_prev = passkey, \
            passkey_prev_until = now() + ($4::bigint * interval '1 hour') \
         WHERE id = $1 RETURNING passkey",
    )
    .bind(auth.id)
    .bind(&new_hash)
    .bind(&new_passkey)
    .bind(domain::passkey_grace_hours())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 改密必须同时失效 5s 状态缓存：must_reset_password 是这条缓存的字段之一，
    // 不失效的话，用户改完密（尤其装机时 root 首次强制改密）最多 5 秒内仍被判
    // 「账号正在使用临时密码」，向导下一步 POST /setup 直接吃 400。
    state.user_status_cache.invalidate(auth.id);
    let _ = rotated;
    state
        .repo
        .audit(
            Some(auth.id),
            "passkey_rotate_on_password_change",
            Some(auth.id),
        )
        .await;
    // 撤销既有 token：nbf 抬到「本次凭证 iat − 1」。require_auth 用 iat<=nbf 判失效，
    // 因此界线为 iat−1 时：本次改密凭证自身已通过认证（不再受检），一切 iat ≤ iat−1
    // 的旧 token 失效；改密后同秒重登的新 token（iat 相同）不受牵连 —— 不误杀合法新登录，
    // 而真正的旧凭证（改密所用的那张）即便 iat 同秒也已在本次请求中消耗，语义无损。
    let _ = sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET nbf = \
         GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(auth.iat - 1)
    .execute(&state.repo.db)
    .await;
    let mut c = state.redis.clone();
    let _: () = redis::AsyncCommands::set_ex(
        &mut c,
        format!("logout_nbf:{}", auth.id),
        auth.iat - 1,
        86400u64,
    )
    .await
    .unwrap_or(());
    state
        .repo
        .audit(Some(auth.id), "password_change", Some(auth.id))
        .await;
    Ok(ok(serde_json::json!({ "changed": true })))
}

#[derive(Deserialize)]
pub(super) struct PasswordChangeReq {
    pub(super) old_password: String,
    pub(super) new_password: String,
}

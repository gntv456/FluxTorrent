//! 敏感动线的凭证复核（step-up）：绑定 passkey 之前必须再认一次本人。
//!
//! 为什么要这一道（P-107）：`/auth/passkeys/begin|finish` 原本只要
//! `require_auth`，于是「拿到一次会话」就等于「可以给这个账号自行注册一把
//! passkey」；而 passkey 登录路径**不问 TOTP**（它本身就是二次因子）——两者
//! 相加，会话被偷的人可为受害者绑好凭证，之后每次都用这把 passkey 直接登录，
//! **2FA 被整条绕过**，且受害者的密码与验证码一次都没被问到。批三十一的 AV
//! 注释里已经写着「一次会话被偷就走完它 = 永久后门」，当时只加了限流。
//!
//! 复核内容按「这个账号实际开了什么」收口：密码必对；`totp_enabled` 时还要
//! 当前窗口（±1）的验证码。这里**不**用登录路径的 90s 防重放
//! （`totp_verify_login`）：用户常常刚用某个码登录完就进面板操作，同码在
//! 30s 窗口内二次出现是正常的，防重放交给本端点自己的速率档。

use actix_web::web;
use data_encoding::BASE32_NOPAD;

use super::totp_verify;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 绑定 passkey 前的复核：`password` 必给且必对，开了 2FA 再要当前码。
/// 文案统一「凭证无效，未绑定 passkey」——不区分「密码错」与「账号不存在」，
/// 也明确告诉调用方这次操作**没有**落任何凭证。
pub(crate) async fn step_up(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    password: &str,
    code: Option<u32>,
    what: &str,
) -> DomainResult<()> {
    if password.is_empty() {
        return Err(DomainError::Validation(format!(
            "{what}需先复核密码（开了 2FA 还要当前验证码）"
        )));
    }
    let row: Option<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT pass_hash, totp_secret, totp_enabled FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((hash, secret, enabled)) = row else {
        return Err(DomainError::Validation(format!("凭证无效，{what}未执行")));
    };
    if !crate::domain::verify_password(&hash, password) {
        state.repo.audit(Some(uid), "stepup_failed", None).await;
        return Err(DomainError::Validation(format!("凭证无效，{what}未执行")));
    }
    if !enabled {
        return Ok(());
    }
    let Some(code) = code else {
        return Err(DomainError::Validation(
            "该账号已开两步验证，请一并提交当前验证码".into(),
        ));
    };
    let b32 = secret.unwrap_or_default();
    let raw = BASE32_NOPAD
        .decode(b32.as_bytes())
        .map_err(|e| DomainError::Validation(format!("密钥损坏: {e}")))?;
    if !totp_verify(&raw, code) {
        state.repo.audit(Some(uid), "stepup_failed", None).await;
        return Err(DomainError::Validation("验证码不正确".into()));
    }
    Ok(())
}

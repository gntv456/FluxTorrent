//! 可插拔验证码驱动（0227）：none / turnstile / recaptcha / hcaptcha。
//!
//! 第三方模式统一走「前端组件拿 token → 注册时后端 siteverify」：
//! - turnstile:  POST https://challenges.cloudflare.com/turnstile/v0/siteverify
//! - recaptcha:  POST https://www.google.com/recaptcha/api/siteverify
//! - hcaptcha:   POST https://api.hcaptcha.com/siteverify
//! 三家请求形状一致（secret + response [+remoteip]），响应都有 success 布尔——
//! 一个通用 verify 函数按 provider 换 URL 即可。
//! none（缺省）= 维持自研算术题（captcha.rs 原路径，零外部依赖）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 已知驱动白名单归一化（P-689 配置健壮性）：库里的值可能来自旧版本、
/// 手工改库或导入包。未知值若原样下发，前端按「第三方组件」渲染而实际
/// 没有可挂的脚本（页面无验证码），后端 siteverify_url 也返回 None
/// ⇒ 静默回落到算术题，但界面没有算术题入口，注册必败且文案误导。
/// 未知/大小写/空白一律回落 none（自研算术题），至少入口可用。
pub fn normalize_provider(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pow" => "pow",
        "turnstile" => "turnstile",
        "recaptcha" => "recaptcha",
        "hcaptcha" => "hcaptcha",
        _ => "none",
    }
}

/// 读当前驱动（键缺失/坏值回落 none）
pub async fn provider(db: &sqlx::PgPool) -> String {
    let raw: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'captcha_provider'), 'none')",
    )
    .fetch_one(db)
    .await
    .unwrap_or_else(|_| "none".into());
    normalize_provider(&raw).to_string()
}

fn siteverify_url(provider: &str) -> Option<&'static str> {
    match provider {
        "turnstile" => {
            Some("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        }
        "recaptcha" => Some("https://www.google.com/recaptcha/api/siteverify"),
        "hcaptcha" => Some("https://api.hcaptcha.com/siteverify"),
        _ => None,
    }
}

/// 注册链路统一入口：按驱动分发校验。
/// - none：走自研算术题（captcha_id + captcha_answer，调用方已有逻辑）
/// - 第三方：body.captcha_token（组件 token）→ 本函数 siteverify
/// 返回 Ok(true) 表示「该请求的验证码校验由本函数完成且通过」；
/// Ok(false) 表示当前是 none 模式（调用方回落算术题路径）。
pub async fn verify(
    state: &AppState,
    provider: &str,
    token: &str,
    ip: &str,
) -> DomainResult<bool> {
    let Some(url) = siteverify_url(provider) else {
        return Ok(false);
    };
    if token.trim().is_empty() {
        return Err(DomainError::Validation(
            "请完成验证码（人机验证组件）后重试".into(),
        ));
    }
    let secret: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'captcha_secret'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    if secret.is_empty() {
        return Err(DomainError::Validation(
            "验证码密钥未配置（后台「验证码密钥」），第三方验证码无法校验"
                .into(),
        ));
    }
    let client = reqwest::Client::new();
    let mut form =
        vec![("secret", secret.as_str()), ("response", token.trim())];
    if !ip.is_empty() {
        form.push(("remoteip", ip));
    }
    let resp: serde_json::Value = client
        .post(url)
        .form(&form)
        .send()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let ok = resp
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !ok {
        return Err(DomainError::Validation("验证码校验未通过，请重试".into()));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::normalize_provider;

    #[test]
    fn unknown_or_dirty_provider_falls_back_to_arithmetic() {
        assert_eq!(normalize_provider("turnstile"), "turnstile");
        assert_eq!(normalize_provider(" POW "), "pow");
        assert_eq!(normalize_provider("Recaptcha"), "recaptcha");
        assert_eq!(normalize_provider("bogus"), "none");
        assert_eq!(normalize_provider(""), "none");
    }
}

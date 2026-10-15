//! 登录/注册请求与展示类型（M01）：LoginReq/RegisterReq/PasswordChangeReq/PublicProfile。
//! 从 auth_http/login.rs 按域拆出（login/register/profile 共用）。

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct LoginReq {
    pub(super) username: String,
    pub(super) password: String,
    #[serde(default)]
    pub(super) totp_code: Option<u32>,
    /// 工作量证明（sha256-pow-v1）：连续失败 ≥2 次起必带，
    /// 由 GET /auth/pow-challenge 签发的 token + 求得的 nonce 解
    #[serde(default)]
    pub(super) pow_token: Option<String>,
    #[serde(default)]
    pub(super) pow_solution: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RegisterReq {
    pub(super) username: String,
    pub(super) email: String,
    pub(super) password: String,
    pub(super) invite_code: String,
    #[serde(default)]
    pub(super) captcha_id: String,
    #[serde(default)]
    pub(super) captcha_answer: i32,
    /// 第三方验证码 token（0227）：captcha_provider 为
    /// turnstile/recaptcha/hcaptcha 时必填，前端组件产出
    #[serde(default)]
    pub(super) captcha_token: String,
    /// 注册页自定义字段值（0186）：key → 值；required 字段缺省时注册拒绝
    #[serde(default)]
    pub(super) fields: std::collections::HashMap<String, serde_json::Value>,
    /// 注册答题票据（0372 M5）：reg_quiz_enabled=yes 时必填，一次性
    #[serde(default)]
    pub(super) quiz_ticket: String,
    #[serde(default)]
    pub(super) quiz_answer: i32,
    /// 注册 PoW（0372）：captcha_provider=pow 时必填（sha256-pow-v1）
    #[serde(default)]
    pub(super) pow_token: String,
    #[serde(default)]
    pub(super) pow_solution: String,
}

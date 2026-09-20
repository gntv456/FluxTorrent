//! 登录/注册请求与展示类型（M01）：LoginReq/RegisterReq/PasswordChangeReq/PublicProfile。
//! 从 auth_http/login.rs 按域拆出（login/register/profile 共用）。

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct PasswordChangeReq {
    pub(super) old_password: String,
    pub(super) new_password: String,
}

#[derive(Deserialize)]
pub(super) struct LoginReq {
    pub(super) username: String,
    pub(super) password: String,
    #[serde(default)]
    pub(super) totp_code: Option<u32>,
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
}

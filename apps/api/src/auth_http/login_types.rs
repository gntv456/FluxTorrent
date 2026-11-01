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
    /// 「记住登录」（2026-11-01）：为真时会话拉长到 REMEMBER_TTL_HOURS。
    /// 取 `Option<Value>` 而非 `bool` 是被本口的双入口逼的——JSON 与表单
    /// POST 都收（0267 NP 生态 takelogin 习惯），而表单里复选框交上来的是
    /// `remember=on`，按 bool 硬解会把一个今天能成功的请求变成 400。
    #[serde(default)]
    pub(super) remember: Option<serde_json::Value>,
}

/// `remember` 的宽松真值判定：JSON 的 `true`/`1`、表单的
/// `on`/`1`/`true`/`yes` 都算勾选；缺省、null、`false`/`0`/空串不勾。
/// 认不出来的形状一律按**不勾**处理——拉长有效期是放宽，
/// 判不准时取窄的那头。
pub(super) fn is_checked(v: &Option<serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        Some(serde_json::Value::String(s)) => {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "1" | "on" | "true" | "yes"
            )
        }
        _ => false,
    }
}

/// 「记住登录」勾选时的会话时长（小时）。720 = 30 天，正好是
/// `jwt_ttl_hours` 那条 SQL 的钳位上限——不另造一个更长的口径，
/// 站长把 jwt_ttl_hours 调到顶时两档自然合流。
const REMEMBER_TTL_HOURS: i64 = 720;

/// 实际会话时长：勾了就取「站点配置」与 30 天的较大者。
/// 调用点必须把返回值同时喂给 JWT 的 exp 与 cookie 的 Max-Age：两处
/// 读同一个变量才不会漂，拆成两个来源就会出现「票还活着、cookie 先
/// 过期」，人被打回登录页还以为是自己密码错了。
pub(super) fn session_ttl(
    remember: &Option<serde_json::Value>,
    base_hours: i64,
) -> i64 {
    if is_checked(remember) {
        base_hours.max(REMEMBER_TTL_HOURS)
    } else {
        base_hours
    }
}

#[cfg(test)]
mod tests {
    use super::session_ttl;
    use serde_json::json;

    fn ttl(v: Option<serde_json::Value>) -> i64 {
        session_ttl(&v, 24)
    }

    /// 缺省与各种「没勾」的形状都必须保持站点配置——判不准时取窄的那头
    #[test]
    fn unchecked_keeps_site_ttl() {
        assert_eq!(ttl(None), 24);
        assert_eq!(ttl(Some(json!(null))), 24);
        assert_eq!(ttl(Some(json!(false))), 24);
        assert_eq!(ttl(Some(json!(0))), 24);
        assert_eq!(ttl(Some(json!(""))), 24);
        assert_eq!(ttl(Some(json!("no"))), 24);
    }

    /// 勾选的三种形状：JSON 布尔（自家前端）、JSON 数字、表单 `on`
    /// （0267 的表单口是给 NP 生态 takelogin 用的，复选框交上来是字符串）
    #[test]
    fn checked_extends_to_thirty_days() {
        assert_eq!(ttl(Some(json!(true))), 720);
        assert_eq!(ttl(Some(json!(1))), 720);
        for s in ["on", "1", "true", "YES", "True"] {
            assert_eq!(ttl(Some(json!(s))), 720, "{s} 应算勾选");
        }
    }

    /// 站长把 jwt_ttl_hours 配得比 30 天还长时，勾选不该反过来改短
    #[test]
    fn never_shortens_a_longer_site_ttl() {
        assert_eq!(session_ttl(&Some(json!(true)), 900), 900);
        assert_eq!(session_ttl(&Some(json!(false)), 900), 900);
    }
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

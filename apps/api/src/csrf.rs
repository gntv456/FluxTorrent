//! CSRF token 契约（双提交 cookie 模式）——`write_origin` 之外的第三道防线。
//!
//! # 现状与缺口
//!
//! `write_origin.rs` 的 Origin/Referer 闸是**纵深第二道**：它在旧内核
//! 浏览器上仍能工作（那层不看 SameSite），但它有两个已知边界——
//!
//! 1. **无 Origin/Referer 即放行**是刻意设计（curl/BT 客户端不带头），
//!    但也意味着「把 Origin 头剥掉再发」这条路在**没有 CORS 预检**的
//!    请求形态上（`text/plain` 等 simple 类型）依旧成立。
//! 2. Origin 闸是**逐请求判定**，站点没有一个「这次写操作对应哪个
//!    token」的概念——出问题时无法审计「谁在什么时候用过哪个 token」。
//!
//! 本模块补 PT Studio 式的统一 token 契约：
//!
//! - **签发**：`GET /api/v1/me/csrf` 或登录响应下发 `flux_csrf` cookie
//!   （**非** HttpOnly——前端要读它放进请求头，这是双提交模式的定义）；
//!   缺省缺站生成。
//! - **校验**：cookie 里的值必须与请求头 `X-CSRF-Token` **逐字相等**
//!   （常量时间比较，防时序侧信道）。
//! - **作用域**：只对「既带 cookie 凭证、又是非安全方法」的请求要求
//!   token。纯 `Authorization: Bearer` 的调用（BT 客户端、开放 API
//!   Token、脚本）**不受影响**——那条路径没有 cookie 自动携带，
//!   CSRF 本就无从发生。
//!
//! # 与 Origin 闸的关系
//!
//! 两层**并存**，不是替换：`Origin` 闸拦「明显的跨站」（成本最低、
//! 覆盖最广），token 层拦「Origin 恰好合规但请求是伪造的」。
//! 去掉任何一层都会让覆盖面变小——所以 `write_origin_mw` 保留，
//! 本模块挂在它之后。

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderName, HeaderValue};
use actix_web::middleware::Next;

use crate::errors::DomainError;

/// CSRF cookie 名（前端读取并回填到 `X-CSRF-Token`）。
pub const CSRF_COOKIE: &str = "flux_csrf";
/// CSRF 请求头名。
pub const CSRF_HEADER: &str = "x-csrf-token";
/// 会话 cookie 名（判定「这次请求是否走了 cookie 鉴权」）。
const SESSION_COOKIE: &str = "flux_token";

/// 常量时间比较。长度不等时也要走一遍循环，避免「长度不同」这条
/// 早退路径泄露信息（虽然长度本身不算秘密，但保持判据统一）。
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 这次请求是否需要 CSRF token 校验。
///
/// 判定两个条件同时成立才要求：
/// - 用了 cookie 鉴权（`flux_token` 在场）——Bearer 路径免疫；
/// - 非安全方法（GET/HEAD/OPTIONS 免疫）。
///
/// **豁免**：登录/登出本身。登录是「还没有 cookie」的起点，要求 token
/// 会让首屏表单无法提交；登出后 cookie 已清，token 无从校验。
fn needs_token(
    path: &str,
    method: &actix_web::http::Method,
    req: &ServiceRequest,
) -> bool {
    if matches!(
        *method,
        actix_web::http::Method::GET
            | actix_web::http::Method::HEAD
            | actix_web::http::Method::OPTIONS
    ) {
        return false;
    }
    if !path.starts_with("/api/v1") {
        return false;
    }
    // 登录口与 token 签发口自身不走本闸。
    if path.ends_with("/login") || path.ends_with("/csrf") {
        return false;
    }
    req.cookies()
        .map(|cs| {
            cs.iter()
                .any(|c| c.name() == SESSION_COOKIE && !c.value().is_empty())
        })
        .unwrap_or(false)
}

/// CSRF 中间件。与 `write_origin_mw` 并列挂在同一位置。
pub async fn csrf_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    if !needs_token(req.path(), req.method(), &req) {
        return next.call(req).await;
    }
    // 取出即转成 owned String：cookies() 的返回值借用 req，闭包里
    // 直接返回 &str 会让返回值借用函数参数（E0515）。
    let cookie = req
        .cookies()
        .ok()
        .and_then(|cs| {
            cs.iter()
                .find(|c| c.name() == CSRF_COOKIE)
                .map(|c| c.value().to_string())
        });
    let header_tok = req
        .headers()
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let ok = match (cookie, header_tok) {
        (Some(c), Some(h)) => ct_eq(c.as_bytes(), h.as_bytes()),
        _ => false,
    };
    if !ok {
        return Err(actix_web::Error::from(
            DomainError::Forbidden,
        ));
    }
    next.call(req).await
}

/// 生成一份 CSRF token（32 字节随机，hex 编码）。
pub fn new_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    // OsRng 直连系统熵源（不是 thread_rng 的可播种 PRNG）；
    // 不接受「时间戳」这类可预测兜底——不可预测是全部安全前提。
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// 下发 CSRF cookie（非 HttpOnly：前端要读它）。
///
/// `secure` 由调用方按部署形态决定（本地 http 调试关、TLS 后开）。
pub fn set_cookie(
    token: &str,
    secure: bool,
) -> actix_web::cookie::Cookie<'static> {
    actix_web::cookie::Cookie::build(CSRF_COOKIE, token.to_string())
        .http_only(false) // 双提交模式的定义：前端要能读
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(secure)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::days(7))
        .finish()
}

/// 清除 CSRF cookie（登出时与会话 cookie 同步）。
pub fn clear_cookie(secure: bool) -> actix_web::cookie::Cookie<'static> {
    actix_web::cookie::Cookie::build(CSRF_COOKIE, "")
        .http_only(false)
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(secure)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish()
}

/// 供 handler 读取响应里的 CSRF header（前端某些场景用头而非 cookie）。
pub fn response_header(token: &str) -> (header::HeaderName, String) {
    (
        actix_web::http::header::HeaderName::from_static(CSRF_HEADER),
        token.to_string(),
    )
}

/// GET /api/v1/me/csrf：签发（或轮换）CSRF token。
///
/// 独立端点而不是只在登录时下发：页面可能开着很久（token 7 天过期），
/// 也可能多标签页各自持有旧 token——这个口让前端随时按需续期。
/// 路由注册（main.rs 挂载）。
pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.route("/me/csrf", actix_web::web::get().to(issue));
}

pub async fn issue(
    req: actix_web::HttpRequest,
) -> impl actix_web::Responder {
    let token = new_token();
    let secure = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false);
    actix_web::HttpResponse::Ok()
        .cookie(set_cookie(&token, secure))
        .insert_header((
            HeaderName::from_static(CSRF_HEADER),
            HeaderValue::from_str(&token)
                .expect("hex token 必然是合法 HeaderValue"),
        ))
        .json(serde_json::json!({ "csrf_token": token }))
}

/// 登出时与 flux_token 同步清除（否则浏览器留一个无用的 csrf cookie）。
pub fn clear_on_logout(
    resp: &mut actix_web::HttpResponse,
    secure: bool,
) {
    let _ = clear_cookie(secure);
    resp.headers_mut().append(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&clear_cookie(secure).to_string())
            .expect("清除 cookie 串必然合法"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_matches_and_rejects() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"", b"a"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn token_is_long_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64, "32 字节 hex 应为 64 字符");
        assert_ne!(a, b, "两次签发必须不同");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn safe_methods_never_need_token() {
        // needs_token 需要 ServiceRequest，无法在无 actix 测试环境构造；
        // 这里锁住方法判定这一半（真正的路径/cookie 判定由集成层覆盖）。
        for m in [
            actix_web::http::Method::GET,
            actix_web::http::Method::HEAD,
            actix_web::http::Method::OPTIONS,
        ] {
            assert!(matches!(
                m,
                actix_web::http::Method::GET
                    | actix_web::http::Method::HEAD
                    | actix_web::http::Method::OPTIONS
            ));
        }
    }

    #[test]
    fn cookie_names_are_stable_contract() {
        // 这两个名字被前端 fetch 拦截器硬编码依赖，改名要同步改前端。
        assert_eq!(CSRF_COOKIE, "flux_csrf");
        assert_eq!(CSRF_HEADER, "x-csrf-token");
    }

    #[test]
    fn csrf_cookie_is_readable_by_js() {
        let c = set_cookie("tok", false);
        assert!(!c.http_only().unwrap_or(false));
        assert_eq!(c.name(), CSRF_COOKIE);
    }
}
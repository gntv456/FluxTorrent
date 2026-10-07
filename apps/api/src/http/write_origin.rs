//! 写方法跨站来源闸（P2，2026-10-06 全站安全审计）。
//!
//! 背景：本站鉴权走 HttpOnly cookie（SameSite=Lax）。Lax 挡住了跨站 POST
//! 自动带 cookie，是 CSRF 的**唯一**防线——但项目明确兼容旧内核浏览器
//! （layout.tsx 为旧 Chrome 加 polyfill），SameSite 属性在这些浏览器上
//! 会被忽略。JSON 路径跨站请求必触发 CORS 预检（Content-Type 非 simple
//! 类型），白名单外 origin 拿不到响应；但 `text/plain` 等 simple 类型
//! 的 POST 不预检，Lax 失效的旧浏览器上仍可跨站打写接口。
//!
//! 判据与 login.rs 的表单闸同源复用（origin_matches）：
//!   - 无 Origin/Referer → 非浏览器客户端（curl/移动端/BT 工具），放行；
//!   - Origin 优先，Referer 兜底（旧浏览器可能只发 Referer）；
//!   - 与 Host 同源或在 CORS_ORIGINS 白名单内才放行，其余 403。
//! 只拦 /api/v1 下的非 GET/HEAD/OPTIONS；tracker announce 不在此路径下，
//! BT 客户端不受影响。

use actix_web::body::MessageBody;
use actix_web::dev::ServiceRequest;
use actix_web::middleware::Next;

pub async fn write_origin_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<actix_web::dev::ServiceResponse<impl MessageBody>> {
    let method = req.method();
    let is_write = !matches!(
        *method,
        actix_web::http::Method::GET
            | actix_web::http::Method::HEAD
            | actix_web::http::Method::OPTIONS
    );
    if !(is_write && req.path().starts_with("/api/v1"))
        || origin_ok(req.request())
    {
        return next.call(req).await;
    }
    // 拒绝分支：转成 DomainError 走统一错误管线（与 setup_gate_mw 同款
    // early-return 形态，避免 impl MessageBody 双分支类型冲突）
    Err(actix_web::Error::from(
        crate::errors::DomainError::Forbidden,
    ))
}

/// 浏览器**实际看到**的 host:port——同源判据要用它，而不是 `Host`。
///
/// 为什么要单独取：本站的浏览器流量一律经 web 容器的同源代理
/// （`apps/web/app/api/[...path]/route.ts`，它显式剥掉 `host` 并把
/// 客户端可见的 host 写进 `x-forwarded-host`）。于是 api 侧看到的
/// `Host` 是上游容器名（如 `api:8080`），永远不等于浏览器发来的
/// `Origin: http://127.0.0.1:3000` ⇒ 二轮审计新加的写来源闸把所有
/// UI 写请求判成跨站，实测连 `POST /auth/login` 都回 403/2003
/// （同一条 curl 不带 Origin 头则 200 —— 闸门只看浏览器才可能发的头，
/// 所以这类失败在 CLI 闸门里永远是绿的，属"假通过"）。
///
/// 安全性没有放宽：`X-Forwarded-Host` 是 fetch 的 forbidden header，
/// 浏览器脚本改不了它，跨站页面伪造不出「与自己 Origin 一致的 XFH」；
/// 只有我们自己的代理会写这个值。非浏览器客户端不发 Origin，本就放行。
/// 多反代链路下取第一段（最靠近客户端的那个 host）。
pub(crate) fn browser_host(req: &actix_web::HttpRequest) -> String {
    let fwd = req
        .headers()
        .get("x-forwarded-host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(',')
        .next()
        .map(str::trim)
        .unwrap_or("");
    if !fwd.is_empty() {
        return fwd.to_string();
    }
    req.headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

/// Origin（优先）/Referer（兜底）存在时必须同源或白名单命中
/// （origin_matches 与 login 表单闸共用实现）。
fn origin_ok(req: &actix_web::HttpRequest) -> bool {
    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let referer = req
        .headers()
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    match (&origin, &referer) {
        (Some(o), _) => crate::auth_http::origin_matches(req, o),
        (None, Some(r)) => crate::auth_http::origin_matches(req, r),
        (None, None) => true, // 非浏览器客户端（工具/BT）
    }
}

//! 登录请求的来源闸（login CSRF 防护）与表单编码识别。
//! 从 login.rs 按域拆出（300 行门禁）；`origin_matches` 由 write_origin
//! 中间件与本闸共用，故经 auth_http 的 `pub(crate) use` 对外暴露。

use actix_web::HttpRequest;

/// 跨站表单登录（login CSRF）防护（0267）。
///
/// 0267 起登录接受 `application/x-www-form-urlencoded`（为兼容 NexusPHP
/// `takelogin.php` 的工具体），但表单 POST **不触发 CORS 预检** —— 任意站点
/// 都能用一个 `<form>` 把访客浏览器「登录成攻击者的账号」，之后访客的浏览与
/// 流量全记到攻击者名下（洗白账号/套取访客行为）。
///
/// 判据：浏览器发 POST 会带 `Origin`，curl / Python / 移动端不会。
///   - 无 Origin            → 非浏览器客户端，放行（工具登录不受影响）
///   - Origin 与本站 Host 同源，或在 CORS_ORIGINS 白名单内 → 放行
///   - 其余（含 `Origin: null` 的沙箱 iframe）→ 拒绝
///
/// 只对**表单**路径生效：JSON 路径跨站必须走预检，CORS 已覆盖，
/// 且本站前端就是 JSON —— 不动它就不会误伤自家登录。
pub(crate) fn origin_allowed(req: &HttpRequest) -> bool {
    let Some(origin) =
        req.headers().get("origin").and_then(|v| v.to_str().ok())
    else {
        return true;
    };
    origin_matches(req, origin)
}

/// 给定 Origin/Referer 值的同源判定（write_origin 中间件与表单闸共用）。
/// 返回 false：空值、`null`（沙箱 iframe）、与本站 Host 不同源且不在
/// CORS_ORIGINS 白名单内。
pub(crate) fn origin_matches(req: &HttpRequest, origin: &str) -> bool {
    let origin = origin.trim();
    if origin.is_empty() || origin == "null" {
        return false;
    }
    // 本机同源：Origin 的 authority 与请求 Host 一致。
    // Referer 兜底形态是完整 URL（带 path/query）：剥到首个 '/' 或 '?' 为止
    // （不剥会把同站 Referer 误判跨站——二轮审计发现，方向是误拒非放行）
    let authority = origin.split_once("://").map_or(origin, |(_, a)| a);
    let authority = authority.split(['/', '?']).next().unwrap_or(authority);
    // 浏览器可见的 host:port（同源代理下不能用 Host，见 write_origin 注释）
    let host = crate::http::write_origin::browser_host(req);
    if !host.is_empty() && authority.eq_ignore_ascii_case(&host) {
        return true;
    }
    // 显式白名单（反代/多域名场景与 CORS 同一份配置）
    std::env::var("CORS_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .any(|o| {
            let o = o.trim().trim_end_matches('/');
            !o.is_empty()
                && o.eq_ignore_ascii_case(origin.trim_end_matches('/'))
        })
}

/// 请求是否为表单编码（login CSRF 防护只对这条路径生效）
pub(crate) fn is_form_body(req: &HttpRequest) -> bool {
    req.headers()
        .get(actix_web::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.trim_start()
                .to_ascii_lowercase()
                .starts_with("application/x-www-form-urlencoded")
        })
        .unwrap_or(false)
}

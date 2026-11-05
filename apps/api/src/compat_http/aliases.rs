//! NP 别名路径薄壳（0267）：让只认 NexusPHP 硬编码路径的老脚本零改造接入。
//!
//! 设计原则：**能转发就不复制**。别名的唯一职责是「路径翻译」，
//! 业务逻辑一律落回既有端点 —— 这样两处口径不可能漂移
//! （历史上 RSS/Torznab 各自内联一份促销标签就是这么漂的）。
//!
//! | 别名 | 行为 |
//! |---|---|
//! | `getrss.php?token=` | 302 → `/api/v1/rss/{token}`（其余参数原样透传） |
//! | `takelogin.php` | 307 → `/api/v1/auth/login`（保留 POST body，见 login 的表单兼容） |
//! | `userdetails.php?token=` | 200 JSON（NP userdetails 字段口径，不含 passkey） |
//! | `details.php?id=` | 302 → 站点详情页 |

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 站点基址（详情页跳转用）：PUBLIC_SITE_URL 优先，否则按请求 Host 拼。
/// 四审 L5：Host 是客户端可控头，未校验直接拼 base 会把攻击者域名写进
/// RSS item 链接/邮件链接（钓鱼面）。字符白名单 + 必须含点号或 localhost，
/// 不合法一律退固定回退值。
pub(crate) fn site_base(req: &HttpRequest) -> String {
    std::env::var("PUBLIC_SITE_URL")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            req.headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|h| sane_host(h))
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://localhost:3000".into())
        })
}

/// Host 白名单校验：字符集（字母数字 . : - [ ] 与 IPv6 字面量）且
/// 含点号或就是 localhost——端口形态 example.com:8080 同样过。
pub(crate) fn sane_host(h: &str) -> bool {
    let ok = h
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b".:-[]_".contains(&b));
    ok && (h.contains('.') || h == "localhost")
}

/// 从 query string 取一个参数（原样返回，不做 URL 解码 —— token 形状
/// 固定；透传其它参数时也必须保持原样）。
fn qs_get(qs: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    qs.split('&')
        .find_map(|kv| kv.strip_prefix(prefix.as_str()))
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// 去掉某个参数后的 query string（保持原有顺序与编码）。
fn qs_without(qs: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    qs.split('&')
        .filter(|kv| !kv.starts_with(prefix.as_str()) && !kv.is_empty())
        .collect::<Vec<_>>()
        .join("&")
}

/// GET /compat/nexusphp/getrss.php?token=xxx → 真 RSS 端点。
/// 旧参数名 `passkey=` 仍可传 token 值，但不接受真实 passkey。
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/getrss.php",
    tag = "兼容层(NP)",
    description = "别名：getrss.php → 302 到 /rss/{token}，其余筛选参数原样\
透传；`token` 或旧字段名 `passkey` 传 fxr_ token。",
    params(
        ("token" = String, Query, description = "fxr_ RSS token（必填）"),
        ("passkey" = String, Query,
         description = "旧字段名，仅接受 fxr_ token"),
    ),
    responses(
        (status = 302, description = "302 重定向到 /rss/{token}"),
        (status = 401, description = "token 形状非法（code 2001）"),
    )
)]
#[get("/compat/nexusphp/getrss.php")]
async fn alias_getrss(req: HttpRequest) -> DomainResult<HttpResponse> {
    let qs = req.query_string();
    let cred = qs_get(qs, "token")
        .or_else(|| qs_get(qs, "passkey"))
        .ok_or(DomainError::Unauthorized)?;
    if !super::super::rss_http::tokens::valid_token_shape(&cred) {
        return Err(DomainError::Unauthorized);
    }
    let rest = qs_without(&qs_without(qs, "token"), "passkey");
    let mut loc = format!("/api/v1/rss/{cred}");
    if !rest.is_empty() {
        loc.push('?');
        loc.push_str(&rest);
    }
    Ok(HttpResponse::Found()
        .insert_header(("location", loc))
        .finish())
}

/// POST /compat/nexusphp/takelogin.php → 307 转发到 `/api/v1/auth/login`。
/// 307 保留方法与 body，所以表单 POST 会原样落到真登录端点
/// （该端点已兼容 `application/x-www-form-urlencoded`，见 auth_http/login.rs）。
#[utoipa::path(
    post,
    path = "/api/v1/compat/nexusphp/takelogin.php",
    tag = "兼容层(NP)",
    description = "别名：takelogin.php → 307 到 /auth/login，保留 POST body；\
登录端点同时接受表单与 JSON。",
    responses(
        (status = 307, description = "307 重定向到 /api/v1/auth/login"),
    )
)]
#[post("/compat/nexusphp/takelogin.php")]
async fn alias_takelogin(req: HttpRequest) -> DomainResult<HttpResponse> {
    let qs = req.query_string();
    let loc = if qs.is_empty() {
        "/api/v1/auth/login".to_string()
    } else {
        format!("/api/v1/auth/login?{qs}")
    };
    Ok(HttpResponse::TemporaryRedirect()
        .insert_header(("location", loc))
        .finish())
}

/// GET /compat/nexusphp/userdetails.php?token=xxx
///
/// 只接受专用 token；字段口径与 `user.json` 完全一致（共用 np_user_json）。
/// Tracker passkey 不再能读取账号数据。
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/userdetails.php",
    tag = "兼容层(NP)",
    description = "别名：userdetails.php → 账号 JSON。`token` 或旧字段名\
 `passkey` 传 fxr_ token；原始 passkey 一律 401；30 次/分钟。",
    params(
        ("token" = String, Query, description = "fxr_ token（必填）"),
        ("passkey" = String, Query,
         description = "旧字段名，仅接受 fxr_ token"),
    ),
    responses(
        (status = 200, description = "NpUser（与 user.json 同形状）"),
        (status = 401, description = "token 无效（code 2001）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/compat/nexusphp/userdetails.php")]
async fn alias_userdetails(
    state: web::Data<std::sync::Arc<AppState>>,
    req: HttpRequest,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    // 空值视同缺省（与 getrss 的 qs_get 同尺）：token=&passkey=fxr_ 时
    // 必须能回落到 passkey 承载的 token，三口径不齐会产生「工具 A 能用
    // 工具 B 401」的玄学。
    let cred = q
        .get("token")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| q.get("passkey").filter(|s| !s.trim().is_empty()))
        .map(|s| s.trim().to_string())
        .ok_or(DomainError::Unauthorized)?;
    if !super::super::rss_http::tokens::valid_token_shape(&cred) {
        return Err(DomainError::Unauthorized);
    }
    super::limit_passkey(&state, &req, "npud", &cred, 30).await?;
    let uid = crate::rss_http::authz::resolve_user_id(&state, &cred)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    let body = super::nexusphp::np_user_json(&state, uid).await?;
    Ok(crate::openapi_http::no_store(ok(body)))
}

#[derive(serde::Deserialize)]
struct DetailsQuery {
    id: i64,
}

/// GET /compat/nexusphp/details.php?id=xxx → 302 到站点详情页。
/// 不伪造 HTML：本站详情页是 RSC 渲染，硬拼一份假 DOM 只会让解析型工具
/// 拿到看似成功实则错位的字段。跳转是诚实的降级。
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/details.php",
    tag = "兼容层(NP)",
    description = "别名：details.php → 302 到站点详情页。不伪造 HTML：\
解析型工具请改用 /open/torrents/{id}。",
    params(("id" = String, Query, description = "种子 id（必填）")),
    responses(
        (status = 302, description = "302 重定向到站点详情页"),
    )
)]
#[get("/compat/nexusphp/details.php")]
async fn alias_details(
    req: HttpRequest,
    q: crate::http::Q<DetailsQuery>,
) -> DomainResult<HttpResponse> {
    Ok(HttpResponse::Found()
        .insert_header((
            "location",
            format!("{}/torrent/{}", site_base(&req), q.id),
        ))
        .finish())
}

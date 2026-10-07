//! NP 别名路径薄壳（0267）：让只认 NexusPHP 硬编码路径的老脚本零改造接入。
//!
//! 设计原则：**能转发就不复制**。别名的唯一职责是「路径翻译」，
//! 业务逻辑一律落回既有端点 —— 这样两处口径不可能漂移
//! （历史上 RSS/Torznab 各自内联一份促销标签就是这么漂的）。
//!
//! | 别名 | 行为 |
//! |---|---|
//! | `getrss.php?passkey=` | 302 → `/api/v1/rss/{passkey}`（其余参数原样透传） |
//! | `takelogin.php` | 307 → `/api/v1/auth/login`（保留 POST body，见 login 的表单兼容） |
//! | `userdetails.php?passkey=` | 200 JSON（NP userdetails 字段口径） |
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

/// 从 query string 取一个参数（原样返回，不做 URL 解码 —— passkey 是
/// 32 位小写字母数字，无需解码；透传其它参数时也必须保持原样）。
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

/// GET /compat/nexusphp/getrss.php?passkey=xxx → 真 RSS 端点。
/// 刷流脚本（auto_feed_js 等）习惯拼 `getrss.php?passkey=`，
/// 此前打这个是 404 —— 现在 302 到 `/api/v1/rss/{passkey}`，其余筛选参数原样带过去。
/// 限流在目标端点（`/rss/{passkey}`）统一计，别名层不重复计数。
#[get("/compat/nexusphp/getrss.php")]
async fn alias_getrss(req: HttpRequest) -> DomainResult<HttpResponse> {
    let qs = req.query_string();
    let passkey = qs_get(qs, "passkey").ok_or(DomainError::Unauthorized)?;
    // 形状校验：passkey 会被拼进 Location 头，畸形输入一律拒
    if !super::valid_passkey(&passkey) {
        return Err(DomainError::Unauthorized);
    }
    let rest = qs_without(qs, "passkey");
    let mut loc = format!("/api/v1/rss/{passkey}");
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

/// GET /compat/nexusphp/userdetails.php?passkey=xxx
///
/// NP 工具惯用「passkey 即身份」读用户数据。本站 passkey 与 tracker /
/// download.php 同源（同一枚凭证已可下载与汇报），因此这里不额外扩大暴露面；
/// 字段口径与 `user.json` 完全一致（共用 np_user_json）。
///
/// 0267 安全补：本端点**必须**限流 —— 此前无限流意味着拿到一枚 passkey
/// 就能无限次读账号数据（打库 + 放大信息面）。现按 passkey 30 次/分钟，
/// 与 download.php 同口径；响应禁缓存（含 passkey，不得被中间层留存）。
#[get("/compat/nexusphp/userdetails.php")]
async fn alias_userdetails(
    state: web::Data<std::sync::Arc<AppState>>,
    req: HttpRequest,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let passkey = q
        .get("passkey")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(DomainError::Unauthorized)?;
    if !super::valid_passkey(&passkey) {
        return Err(DomainError::Unauthorized);
    }
    super::limit_passkey(&state, &req, "npud", &passkey, 30).await?;
    // 判据收在视图 user_by_passkey（改密/重置后的宽限窗内旧钥仍可解析）
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM user_by_passkey WHERE passkey = $1 AND status < 2",
    )
    .bind(&passkey)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::Unauthorized);
    };
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
#[get("/compat/nexusphp/details.php")]
async fn alias_details(
    req: HttpRequest,
    q: web::Query<DetailsQuery>,
) -> DomainResult<HttpResponse> {
    Ok(HttpResponse::Found()
        .insert_header((
            "location",
            format!("{}/torrent/{}", site_base(&req), q.id),
        ))
        .finish())
}

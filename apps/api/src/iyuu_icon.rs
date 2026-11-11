//! IYUU 辅种面板的站点图标代理。
//!
//! **为什么不像 NexusPHP 那样直接热链**：NP 的 `getSiteIcon()` 就是
//! `<img src="https://{对方域名}/favicon.ico">`——用户浏览器直连对方站，
//! 等于把访问者的 IP 和 Referer（本站地址）送给十几个第三方。这里改成
//! 服务端拉一次、缓存 7 天，前端只连我们自己的域名。
//!
//! 四道护栏（都不可省）：
//! 1. **SSRF**：域名来自 IYUU 站点表（半可信），复用 0389 给运维 webhook
//!    写的 `egress_allowed`，fail-closed；内网/环回/非公网一律拒。
//!    **重定向逐跳复核**：只校验起始 URL 挡不住「302 到内网」，所以
//!    reqwest 的默认跟随被换成 custom policy，每一跳都要再过一次闸。
//! 2. **体积**：`content-length` 先看，读回来再核一次 ≤64KB——对方返回
//!    一坨几百 MB 的东西不能进内存更不该进 Redis。
//! 3. **类型**：只收 `image/*`（兼容 `application/octet-stream`，
//!    很多站的 .ico 就是这个类型），其余当没有。
//! 4. **负缓存**：拿不到也记 24h，避免一个页面十几个失效 sid 每次都
//!    各自等 3 秒超时，把详情页拖慢。
//!
//! 拿不到就回 204：前端 `<img onError>` 本来就会隐藏，缺图不该冒错。

use actix_web::{get, web, HttpRequest, HttpResponse};
use redis::AsyncCommands;
use reqwest::redirect::{Attempt, Policy};

use crate::domain::webhook::{egress_allowed, Egress};
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

/// 图标缓存时长：7 天。favicon 基本不变，没必要勤刷。
const TTL: u64 = 7 * 86_400;
/// 「确认拿不到」的缓存时长：1 天。别把对方站临时故障记死。
const EMPTY_TTL: u64 = 86_400;
/// 单张上限 64KB。
const MAX_BYTES: u64 = 64 * 1024;
/// 出站超时（秒）。
const TIMEOUT_SECS: u64 = 3;
/// 最多跟随几跳重定向。
const MAX_HOPS: usize = 3;
/// 负缓存哨兵；真实 content-type 永远不会是这个值。
const EMPTY: &str = "-";

#[utoipa::path(
    get,
    path = "/api/v1/iyuu/icon/{sid}",
    tag = "辅种(crossseed)",
    description = "IYUU 站点图标代理（会话鉴权）。服务端拉取并缓存 7 天，\
            避免用户浏览器直连第三方站点；取不到返回 204。",
    params(("sid" = i64, description = "IYUU 站点 id")),
    responses(
        (status = 200, description = "图标字节"),
        (status = 204, description = "无图标（前端隐藏）"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/iyuu/icon/{sid}")]
pub(crate) async fn iyuu_icon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    match cache_get(&state, sid).await {
        Cached::Hit(ct, body) => return Ok(reply(ct, body)),
        Cached::Empty => return Ok(no_content()),
        Cached::Miss => {}
    }
    // 插件关了就不该有任何出站，即使有人手工打这个端点。
    let conf = crate::iyuu_bridge::conf(&state).await;
    if !conf.enabled {
        return Ok(no_content());
    }
    let host = match crate::iyuu_bridge::host_of(&state, &conf, sid).await {
        Some(h) => h,
        None => {
            cache_set_empty(&state, sid).await;
            return Ok(no_content());
        }
    };
    let url = format!("https://{host}/favicon.ico");
    // 域名虽来自 IYUU 表，仍按不可信处理：这条通路能出网，就必须过闸。
    if let Egress::Deny(reason) = egress_allowed(&url, &[]) {
        tracing::warn!(host = %host, reason = %reason, "拒绝拉取站点图标");
        cache_set_empty(&state, sid).await;
        return Ok(no_content());
    }
    let Some((ct, body)) = fetch(&url).await else {
        cache_set_empty(&state, sid).await;
        return Ok(no_content());
    };
    cache_set(&state, sid, &ct, &body).await;
    Ok(reply(ct, body))
}

/// 每一跳都重新过 SSRF 闸。
///
/// reqwest 默认跟随 10 跳，只校验起始 URL 的话，对方站一个
/// `302 http://169.254.169.254/...` 就把闸绕过去了。这里显式接管，
/// 每跳重查、跳数封顶；任一跳被拒就停下（结果当成没图标）。
fn guarded_redirect() -> Policy {
    Policy::custom(|attempt: Attempt<'_>| {
        if attempt.previous().len() >= MAX_HOPS {
            return attempt.stop();
        }
        match egress_allowed(attempt.url().as_str(), &[]) {
            Egress::Allow => attempt.follow(),
            Egress::Deny(_) => {
                tracing::warn!(url = %attempt.url(), "拒绝跟随图标重定向");
                attempt.stop()
            }
        }
    })
}

/// 拉一次 favicon。失败/超限/类型不对都返回 None。
async fn fetch(url: &str) -> Option<(String, Vec<u8>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .redirect(guarded_redirect())
        .build()
        .ok()?;
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if resp.content_length().unwrap_or(0) > MAX_BYTES {
        return None;
    }
    let raw = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !is_image(&raw) {
        return None;
    }
    let body = resp.bytes().await.ok()?.to_vec();
    // 无 content-length（chunked）时只能读完再核，超限就丢。
    if body.is_empty() || body.len() as u64 > MAX_BYTES {
        return None;
    }
    Some((normalize_ct(&raw), body))
}

/// 只收图片；`application/octet-stream` 与空类型放行（很多站的 .ico
/// 就是这么发的，浏览器照样能显示）。
fn is_image(ct: &str) -> bool {
    let ct = ct.trim().to_ascii_lowercase();
    ct.starts_with("image/")
        || ct.starts_with("application/octet-stream")
        || ct.is_empty()
}

/// 空类型 / octet-stream 归一成 icon mime。
///
/// 直接把空串塞进 `content_type()` 会发出 `content-type: ` 空值，
/// 浏览器可能因此把图当文本而不渲染；归一后 `<img>` 一定能显示。
fn normalize_ct(ct: &str) -> String {
    let ct = ct.trim().to_ascii_lowercase();
    if ct.starts_with("image/") {
        ct
    } else {
        "image/x-icon".to_string()
    }
}

/// 缓存三态：有图 / 确认没图 / 没查过。
enum Cached {
    Hit(String, Vec<u8>),
    Empty,
    Miss,
}

/// 缓存读。两个键各自过期，只剩 ct 没有字节时按没查过处理。
async fn cache_get(state: &AppState, sid: i64) -> Cached {
    let mut c = state.redis.clone();
    let ct: Option<String> = match c.get(format!("iyuu:icon:{sid}:ct")).await {
        Ok(v) => v,
        Err(_) => return Cached::Miss,
    };
    let Some(ct) = ct else {
        return Cached::Miss;
    };
    if ct == EMPTY {
        return Cached::Empty;
    }
    let body: Option<Vec<u8>> =
        c.get(format!("iyuu:icon:{sid}")).await.ok().flatten();
    match body {
        Some(b) if !b.is_empty() => Cached::Hit(ct, b),
        // 半截缓存要重新拉，别误判成「确认没图」。
        _ => Cached::Miss,
    }
}

/// 缓存写。失败不影响本次返回，只是下次还要再拉一遍。
async fn cache_set(state: &AppState, sid: i64, ct: &str, body: &[u8]) {
    let mut c = state.redis.clone();
    let _: Result<(), _> =
        c.set_ex(format!("iyuu:icon:{sid}:ct"), ct, TTL).await;
    let _: Result<(), _> =
        c.set_ex(format!("iyuu:icon:{sid}"), body, TTL).await;
}

/// 负缓存：只写 ct 哨兵，顺手删掉可能残留的旧字节。
async fn cache_set_empty(state: &AppState, sid: i64) {
    let mut c = state.redis.clone();
    let _: Result<(), _> = c
        .set_ex(format!("iyuu:icon:{sid}:ct"), EMPTY, EMPTY_TTL)
        .await;
    let _: Result<(), _> = c.del(format!("iyuu:icon:{sid}")).await;
}

/// 图标响应。缓存 7 天，与 Redis TTL 对齐。
fn reply(ct: String, body: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ct)
        .insert_header(("cache-control", "public, max-age=604800"))
        .body(body)
}

/// 没有图标：204，前端 `onError` 隐藏图片，不留破图。
fn no_content() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

#[cfg(test)]
mod tests;

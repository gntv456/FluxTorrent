//! 海报外链代理（/works/poster-proxy）：豆瓣图床 2026-11 起无 Referer
//! 直接 418（旧版按 Referer 防盗链、no-referrer 可绕；新版反过来了——
//! 必须带豆瓣域 Referer）。前端 `PosterImage` 的 referrerPolicy=
//! no-referrer 从「规避防盗链的正解」变成死路，聚合海报墙全挂。
//!
//! 修法：海报一律走本代理出站——api 注入 `Referer: https://m.douban.com/`
//! 后转给图床，响应原样流回。只白名单图床域（doubanio / image.tmdb.org），
//! 不当开放代理用；无需登录（海报是公开资源，登录墙只会让首屏更空）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};

/// 图床域白名单：host 精确匹配这些后缀才放行。
const IMG_HOST_SUFFIXES: &[&str] = &[".doubanio.com", "image.tmdb.org"];

/// 每张图出站拉取上限：海报 w500 档通常 <200KB，2MB 已是宽裕；
/// 防止把白名单域上的大文件当管道搬运。
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

fn host_allowed(u: &reqwest::Url) -> bool {
    let Some(host) = u.host_str() else {
        return false;
    };
    IMG_HOST_SUFFIXES.iter().any(|s| host.ends_with(s))
}

/// 代理出站客户端：连接池走全局（每请求 new Client 会攒句柄）。
fn poster_client() -> reqwest::Client {
    // 超时口径：图床抖动不该拖死页面（前端 onError 有占位兜底），
    // 5s 连接 + 10s 整体拿不到就放手。
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

/// 转发时带的 Referer 按图床分派：豆瓣认 douban 系域，tmdb 无所谓。
fn referer_for(u: &reqwest::Url) -> &'static str {
    if u.host_str().is_some_and(|h| h.ends_with(".doubanio.com")) {
        "https://m.douban.com/"
    } else {
        "https://www.themoviedb.org/"
    }
}

#[get("/works/poster-proxy")]
pub(crate) async fn poster_proxy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let _ = &req; // 显式保留签名位（与域内端点一致）；本端点不校验登录
    let Some(url) = q.get("url") else {
        return Err(DomainError::Validation("缺少 url 参数".into()));
    };
    let parsed: reqwest::Url = url
        .parse()
        .map_err(|_| DomainError::Validation("url 无效".into()))?;
    if parsed.scheme() != "https" || !host_allowed(&parsed) {
        return Err(DomainError::Validation(
            "仅支持图床白名单域（doubanio / image.tmdb.org）".into(),
        ));
    }
    // 小缓存（Redis 10 分钟）：海报墙一页几十张，翻页来回重复拉同一批
    let cache_key = format!("flux:poster_px:{}", simple_hash(url));
    if let Some(cached) = cache_get(&state, &cache_key).await {
        return Ok(image_response(cached.0, cached.1));
    }
    let resp = poster_client()
        .get(parsed.clone())
        .header("Referer", referer_for(&parsed))
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !resp.status().is_success() {
        // 图床 4xx/5xx 原样透传状态，前端 PosterImage onError 走占位
        return Err(DomainError::NotFound(0));
    }
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let body: Vec<u8> = resp
        .bytes()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .to_vec();
    if body.len() > MAX_IMAGE_BYTES {
        return Err(DomainError::Validation("图片超过 2MB 上限".into()));
    }
    if !ct.starts_with("image/") {
        return Err(DomainError::Validation("非图片内容类型".into()));
    }
    cache_put(&state, &cache_key, &ct, &body).await;
    Ok(image_response(ct, body))
}

fn image_response(ct: String, body: Vec<u8>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ct)
        .insert_header((
            actix_web::http::header::CACHE_CONTROL,
            "public, max-age=600",
        ))
        .body(body)
}

/// URL 稳定短哈希（仅作缓存键，不承安全职责——白名单在上面已判）。
fn simple_hash(s: &str) -> String {
    // FNV-1a 64：无密码学需求，快且无依赖
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

async fn cache_get(state: &AppState, key: &str) -> Option<(String, Vec<u8>)> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let (ct, data): (String, Vec<u8>) =
        AsyncCommands::get(&mut c, key).await.ok()?;
    Some((ct, data))
}

async fn cache_put(state: &AppState, key: &str, ct: &str, data: &[u8]) {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    // 二元组序列化用 redis 的 bulk 写法：SETEX 一个两元素数组
    let _: Result<(), _> = AsyncCommands::set_ex(
        &mut c,
        key,
        (ct.to_string(), data.to_vec()),
        600,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_whitelist() {
        let ok1: reqwest::Url =
            "https://img3.doubanio.com/a.jpg".parse().unwrap();
        let ok2: reqwest::Url =
            "https://image.tmdb.org/t/p/w500/x.jpg".parse().unwrap();
        let bad1: reqwest::Url = "https://evil.com/a.jpg".parse().unwrap();
        let bad2: reqwest::Url =
            "https://img3.doubanio.com.evil.com/a.jpg".parse().unwrap();
        assert!(host_allowed(&ok1) && host_allowed(&ok2));
        assert!(!host_allowed(&bad1));
        assert!(!host_allowed(&bad2), "后缀匹配须锚定 host 末尾");
    }
}

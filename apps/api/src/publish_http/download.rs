//! 下载（M05）：GET /torrents/{id}/download 动态生成 .torrent。
//! 从 publish_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

#[get("/torrents/{id}/download")]
pub async fn download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    use actix_web::body::BoxBody;

    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    // 付费下载（0086）：免费/发布者/已购直接放行，否则扣费（余额不足拦截）
    torrents::charge_for_download(&state.repo.db, auth.id, torrent_id).await?;
    let body = build_torrent_bytes(&state, auth.id, torrent_id).await?;
    let mut resp = HttpResponse::with_body(
        actix_web::http::StatusCode::OK,
        BoxBody::new(body),
    );
    resp.headers_mut().insert(
        actix_web::http::header::CONTENT_TYPE,
        actix_web::http::header::HeaderValue::from_static(
            "application/x-bittorrent",
        ),
    );
    Ok(resp)
}

/// 构建给指定用户的 .torrent 字节（注入本站 announce + passkey + private=1）。
/// 网页下载 / NP 兼容下载（compat_http）/ 临时凭证下载共用；鉴权与下载闸门由调用方先行完成。
/// announce 地址来源：站点设定 announce_url / https_announce_url 优先（设定页可改），
/// PUBLIC_TRACKER_URL 环境变量兜底。配置了 https 时首选加密汇报，http 作 BEP12 回退。
pub async fn build_torrent_bytes(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    torrent_id: i64,
) -> DomainResult<Vec<u8>> {
    let raw: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT f.raw FROM torrent_files f \
         JOIN torrents t ON t.id = f.torrent_id \
         WHERE f.torrent_id = $1 AND t.approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(raw) = raw else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let user = state
        .repo
        .find_user_by_id(user_id)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    // 下载闸门：与 tracker announce 的 left>0 拦截同口径——被停下载/挂起账号
    // 不应还能提前拿到 .torrent 文件
    let (download_enabled, suspended): (bool, bool) = sqlx::query_as(
        "SELECT download_enabled, suspended FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    if suspended {
        return Err(DomainError::Forbidden);
    }
    if !download_enabled {
        return Err(DomainError::Validation(
            "您的下载权限已被暂停，请联系管理组".into(),
        ));
    }
    // info dict 不动 → info_hash 与上传时一致（M05）
    async fn setting(db: &sqlx::PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
    }
    let env_host = std::env::var("PUBLIC_TRACKER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7070".into())
        .trim_end_matches('/')
        .to_string();
    // 审计修复（P0）：历史默认 announce_url 自带尾部 "/announce"（0034 迁移），拼接后
    // 生成 ".../announce/announce/<passkey>" 双重路径 + 错端口，真实客户端必然 404。
    // 这里剥掉尾部 /announce 双保险（迁移 0080 已同时纠正站点设定值本身）。
    let strip_announce = |v: String| -> String {
        let v = v.trim().trim_end_matches('/').to_string();
        match v.strip_suffix("/announce") {
            Some(s) => s.to_string(),
            None => v,
        }
    };
    let base_http = strip_announce(
        setting(&state.repo.db, "announce_url")
            .await
            .unwrap_or(env_host),
    );
    let announce = match setting(&state.repo.db, "https_announce_url").await {
        Some(https) if https != base_http => {
            format!("{}/announce/{}", strip_announce(https), user.passkey)
        }
        _ => format!("{base_http}/announce/{}", user.passkey),
    };
    // http 回退仅在与首选不同时下发（BEP12 单 tier：失败自动降级，不支持 TLS 的老客户端可用）
    let mut fallbacks = Vec::new();
    if !announce.starts_with(&format!("{base_http}/")) {
        fallbacks.push(format!("{base_http}/announce/{}", user.passkey));
    }
    // UDP tracker（BEP15）：默认不启用。私有站的计费身份靠 passkey 随 URL path 传递——
    // HTTP announce（BEP3）天然支持；UDP 包格式没有 path，标准客户端（libtorrent/
    // qBittorrent）不会附带 passkey，UDP tier 只对本站扩展约定的客户端可用。
    // NexusPHP 系站点全部走 HTTP announce，这也是私有 tracker 的行业惯例。
    // 需要时显式设 TRACKER_UDP_URL=udp://host:port 追加 tier（客户端失败后按 BEP12
    // 降级 HTTP 回退，不再默认推断给所有客户端强加一个必然失败的 UDP tier）。
    let udp_url = std::env::var("TRACKER_UDP_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();
    if !udp_url.is_empty() {
        fallbacks.push(format!(
            "{}/{}",
            udp_url.trim_end_matches('/'),
            user.passkey
        ));
    }
    crate::bencode::build_download_torrent(&raw, &announce, &fallbacks)
        .map_err(DomainError::TorrentInvalid)
}

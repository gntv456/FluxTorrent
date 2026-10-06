//! 下载（M05）：GET /torrents/{id}/download 动态生成 .torrent。
//! 从 publish_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, throttle};
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
    // P2（2026-10-06 安全审计）：下载是「动态 bencode 重编码 + 扣费事务 +
    // 读原始种子文件」的高成本路径，登录用户可循环打——加 20 次/分钟
    // per-user 限流（正常使用远够：浏览器单次点击 1 次，RSS 走独立
    // passkey 通道不受影响）。
    throttle(&state, format!("dl:{}", auth.id)).await?;
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
        // 区分两种 404（评审 P1-4）：种子行不在 = 真不存在；行在但 torrent_files
        // 原文丢失 = 站点数据异常——统一报「资源不存在」会让用户误以为被封权，
        // 详情页一切正常却下载 404 的场景必须给出可行动的提示。
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1)",
        )
        .bind(torrent_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if exists {
            return Err(DomainError::Validation(
                "种子文件缺失（原始 .torrent 未存档），请联系管理组补档".into(),
            ));
        }
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
    // 附加 tracker（多域多 tracker）：后台 Tracker URL 管理表按 priority
    // 追加 tier——多域名站每个域名各配一条，客户端按 BEP12 轮换。
    // 过滤口径：enabled=false 不发；纯内网地址（127.0.0.1/localhost）不发
    // （防误配进公网种子）；与已选地址重复（剥 /announce 后同根）跳过；
    // 地址须带 scheme（http(s):// 或 udp://），站长填裸域名时拒绝入库
    // 由管理端校验承担，这里只兜底跳过。
    {
        let extra: Vec<String> = sqlx::query_scalar(
            "SELECT url FROM tracker_urls WHERE enabled \
             ORDER BY priority, id",
        )
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        let mut known_roots: Vec<String> = vec![
            announce.clone(),
            fallbacks.iter().cloned().collect::<Vec<_>>().join("§"),
        ];
        known_roots.retain(|s| !s.is_empty());
        let root_of = |u: &str| -> String {
            // 剥掉尾部 /announce/<passkey>，保留 scheme://host[:port] 根
            let u = u.trim().trim_end_matches('/');
            let (base, _) = u
                .rsplit_once('/')
                .map(|(b, _)| (b.to_string(), String::new()))
                .unwrap_or((u.to_string(), String::new()));
            base
        };
        for url in extra {
            let url = url.trim().trim_end_matches('/').to_string();
            if url.is_empty() {
                continue;
            }
            let lower = url.to_lowercase();
            if !(lower.starts_with("http://")
                || lower.starts_with("https://")
                || lower.starts_with("udp://"))
            {
                continue;
            }
            if lower.contains("127.0.0.1") || lower.contains("localhost") {
                continue;
            }
            let full = if lower.starts_with("udp://") {
                format!("{url}/{}", user.passkey)
            } else {
                // 允许站长填到 /announce 根或裸域名，统一剥后拼
                let base = strip_announce(url.clone());
                format!("{base}/announce/{}", user.passkey)
            };
            let root = root_of(&full);
            if known_roots.iter().any(|k| k.contains(&root)) {
                continue;
            }
            known_roots.push(full.clone());
            fallbacks.push(full);
        }
    }
    crate::bencode::build_download_torrent(&raw, &announce, &fallbacks)
        .map_err(DomainError::TorrentInvalid)
}

//! 下载（M05）：GET /torrents/{id}/download 动态生成 .torrent。
//! 从 publish_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, throttle_limited};
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
    // P-61：以前这 20 只存在于注释里，实际是登录档 5 次/分——批量收种的
    // 第 6 次点击就 429。现在显式传 20。
    throttle_limited(&state, format!("dl:{}", auth.id), 20).await?;
    let torrent_id = path.into_inner();
    // 资格闸门排在扣费之前（P-162）：付费种 + 被停下载/挂起账号原先会先
    // 扣魔力、写「已购」，再由 build_torrent_bytes 拒发 —— 钱当场花了、
    // 文件没拿到，重试还被已购记录放行扣费段。
    torrents::ensure_downloadable(&state.repo.db, auth.id).await?;
    // 付费下载（0086）：免费/发布者/已购直接放行，否则扣费（余额不足拦截）
    torrents::charge_for_download(&state.repo.db, auth.id, torrent_id).await?;
    let body = build_torrent_bytes(&state, auth.id, torrent_id).await?;
    // P-299：存盘名。批量口早就是 `{种名}-{id}.torrent`，单击口此前
    // 只带 Content-Type，浏览器按 URL 末段落盘成 `download`。
    let name =
        crate::repo::torrent_download::torrent_name(&state.repo, torrent_id)
            .await;
    let label = format!(
        "{}-{torrent_id}.torrent",
        crate::archive::safe_name(name.as_deref().unwrap_or("torrent"))
    );
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
    if let Ok(v) = actix_web::http::header::HeaderValue::from_str(
        &super::attachment::content_disposition(&label),
    ) {
        resp.headers_mut()
            .insert(actix_web::http::header::CONTENT_DISPOSITION, v);
    }
    // 响应体内嵌本人 announce（含 passkey）：与 download.php / 凭证下载
    // 同一口径禁缓存（P-163 —— 同一份字节，另两路口都带，唯独网页口漏了）
    Ok(crate::openapi_http::no_store(resp))
}

/// C1（可乐站对标 2026-10-16）：会话口签发临时下载直链——网页列表
/// 「复制下载直链」入口。compat 层 `POST /downloads/keys` 只认
/// `Authorization: Token fxo_…`（第三方开放 API），浏览器会话（Bearer JWT +
/// HttpOnly cookie）调不通；这里按 `require_auth` 会话口径复用同一签发内核
/// （`issue_download_key`：过审存在性 + 资格/付费三道闸），限流也与 compat
/// 签发口共键同档（10 次/分钟，fail-close）。
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/download-link",
    tag = "种子",
    description = "会话口签发 30 分钟临时下载直链（列表「复制下载直链」用；\
与开放 API 的 POST /downloads/keys 同一签发内核与闸门）。",
    params(("id" = i64, Path, description = "种子 id")),
    responses(
        (status = 200, description = "DownloadKey：key + download_url"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "下载闸门拒绝（停下载/挂起/付费未购）"),
        (status = 404, description = "种子不存在或未过审（code 1004）"),
        (status = 429, description = "签发限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/torrents/{id}/download-link")]
pub async fn download_link(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    const TTL: i64 =
        crate::compat_http::download_keys::DOWNLOAD_KEY_TTL_MINUTES;
    let auth = require_auth(&req, &state).await?;
    crate::http::rate_window(
        &state,
        &format!("dlkey:issue:{}", auth.id),
        10,
        60,
    )
    .await?;
    let torrent_id = path.into_inner();
    let (plain, expires) =
        crate::compat_http::download_keys::issue_download_key(
            &state, auth.id, torrent_id,
        )
        .await?;
    // 响应含一次性凭证明文：禁缓存（与 compat 层同口径）
    Ok(crate::openapi_http::no_store(crate::dto::ok(
        serde_json::json!({
            "key": plain,
            "expires_at": expires,
            "ttl_minutes": TTL,
            "download_url":
                format!("/api/v1/downloads/{torrent_id}?token={plain}"),
            "hint": "凭证与用户+种子绑定；明文仅显示一次",
        }),
    )))
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
    let raw = crate::repo::torrent_download::raw_torrent_file(
        &state.repo,
        torrent_id,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(raw) = raw else {
        // 区分两种 404（评审 P1-4）：种子行不在 = 真不存在；行在但 torrent_files
        // 原文丢失 = 站点数据异常——统一报「资源不存在」会让用户误以为被封权，
        // 详情页一切正常却下载 404 的场景必须给出可行动的提示。
        let exists = crate::repo::torrent_download::torrent_exists(
            &state.repo,
            torrent_id,
        )
        .await;
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
    // 下载闸门：判据只有一份（`torrents::ensure_downloadable`）——被停下载/
    // 挂起账号不应还能提前拿到 .torrent 文件。三条下载路都在扣费**之前**
    // 调它，这里再兜一次：将来若有新调用方漏判，最坏是拒单变晚，不是放行。
    crate::torrents::ensure_downloadable(&state.repo.db, user_id).await?;
    // info dict 不动 → info_hash 与上传时一致（M05）
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
        crate::repo::settings::read_base(&state.repo, "announce_url")
            .await
            .unwrap_or(env_host),
    );
    let announce = match crate::repo::settings::read_base(
        &state.repo,
        "https_announce_url",
    )
    .await
    {
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
        let extra =
            crate::repo::torrent_download::extra_trackers(&state.repo).await;
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
        let own_authority = authority_of(&announce);
        for (url, carry_key) in extra {
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
            // 凭据外发守卫（审计 10-07 P1-7）：旧版对每条附加 tracker 都无条件拼
            // `/{passkey}`，站长一旦把友站或公网 tracker 填进这个框，**全体用户**的
            // 32 位 passkey 就被写进下发的 .torrent 并持续外发（对方可据此归因
            // 用户在本站的全部做种行为，并能以其身份 announce）。
            // 现在只有两种情况带密钥：地址属于本站，或该条显式勾了 with_passkey。
            let ours = authority_of(&url) == own_authority;
            let full = if lower.starts_with("udp://") {
                if carry_key || ours {
                    format!("{url}/{}", user.passkey)
                } else {
                    url.clone()
                }
            } else {
                // 允许站长填到 /announce 根或裸域名，统一剥后拼
                let base = strip_announce(url.clone());
                if carry_key || ours {
                    format!("{base}/announce/{}", user.passkey)
                } else {
                    format!("{base}/announce")
                }
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

/// 取 URL 的 authority（host[:port]，小写、去 scheme）。
pub(crate) fn authority_of(u: &str) -> String {
    let rest = u
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("udp://")
        .trim_start_matches("tracker://");
    let host = rest.split('/').next().unwrap_or(rest);
    host.trim_end_matches('/').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::authority_of;

    #[test]
    fn authority_matches_whatever_path_the_operator_pasted() {
        // 站长可能填裸域名、/announce、或带 announce+passkey 的完整链
        let a = authority_of("https://tracker.example.com:7070/announce/abc");
        assert_eq!(a, "tracker.example.com:7070");
        assert_eq!(
            authority_of("https://TRACKER.example.com:7070/"),
            "tracker.example.com:7070"
        );
        assert_eq!(
            authority_of("udp://t.example.com:6969"),
            "t.example.com:6969"
        );
        assert_eq!(authority_of("https://t.example.com"), "t.example.com");
        assert_ne!(
            authority_of("https://friend-site.org/announce"),
            authority_of("https://tracker.example.com/announce/x")
        );
    }
}

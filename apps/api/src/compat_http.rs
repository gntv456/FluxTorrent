//! 生态兼容层（0069，对照 _doc/主流PT架构横向对比与借鉴.md v2 §4）：
//!
//! 背景刷流/辅种工具圈把站点分为 `nexusphp | gazellepw | unit3d | tnode | discuz | mtorrent`
//! 六种「架构方言」（ptool 官方 README），自研架构若不暴露兼容形状的 API 即零适配。
//! 本模块做三件事：
//!   ① `/compat/meta`                     —— 架构自描述（适配器作者的接入入口）
//!   ② `/compat/nexusphp/*`               —— NexusPHP 字段口径的只读 JSON + download.php 形状下载
//!   ③ `/downloads/keys` + `/downloads/{id}` —— 30 分钟临时下载凭证（与长期 API Token 解耦）
//!
//! 鉴权：开放 API Token（`Authorization: Token fxo_...`，复用 openapi_http::require_token，
//! 含时效/撤销/独立限流）；download.php 与凭证下载走 passkey / 一次性短 token ——
//! 与 NexusPHP 生态工具的既有习惯一致。

pub(crate) mod aliases;
mod common;
mod download_keys;
mod nexusphp;
mod ptpp;

// 0267：passkey 维度限流与形状校验 —— RSS 等对外端点共用（避免各自实现漂移）
pub(crate) use common::{limit_passkey, valid_passkey};

use actix_web::{get, HttpResponse};

use crate::dto::ok;
use crate::errors::DomainResult;

/// 架构自描述：第三方适配器（ptool / PT-Plugin-Plus / 脚本作者）的接入入口。
/// 无需鉴权 —— 只包含协议形状，不含任何用户数据。
///
/// 0267：补齐此前**漏报**的能力（Torznab / RSS / 开放数据 / 别名路径）。
/// 机器可发现性也是契约的一部分 —— 工具作者照着 `/compat/meta` 集成，
/// 漏报等于「这个站没有这个能力」。
#[get("/compat/meta")]
async fn compat_meta() -> DomainResult<HttpResponse> {
    Ok(ok(serde_json::json!({
        "name": "FluxTorrent",
        "architecture": "fluxtorrent",
        "compat": { "nexusphp": true, "torznab": true, "rss": true },
        "api_root": "/api/v1",
        "auth": {
            "token": "Authorization: Token <fxo_...>（网页端「我的 → API Token」签发，180 天有效，可 /me/tokens/refresh 续期）",
            "apikey": "同上一枚 Token，可用 ?apikey= 传（Prowlarr/Jackrabbit 等客户端习惯）",
            "passkey": "download.php / getrss.php / tracker / userdetails.php 使用 passkey 查询参数（与 NexusPHP 工具习惯一致）"
        },
        "capabilities": {
            "user_info": true,
            "search": true,
            "rss": true,
            "torznab": true,
            "download_by_passkey": true,
            "download_temp_key": true,
            "incremental_feed": true,
            "upload": true,
            "irc_announce": false
        },
        "endpoints": {
            "user": "/api/v1/compat/nexusphp/user.json",
            "torrents": "/api/v1/compat/nexusphp/torrents.json?page=1",
            "torrent_detail": "/api/v1/compat/nexusphp/torrent/{id}.json",
            "download": "/api/v1/compat/nexusphp/download.php?id={id}&passkey={passkey}",
            "download_key": "POST /api/v1/downloads/keys {torrent_id} → 30 分钟临时凭证",
            "ptpp_user_info": "/api/v1/plugins/ptppUserInfo（PT-Plugin-Plus 字段口径聚合端点）",
            "rss": "/api/v1/rss/{passkey}（item 含 enclosure 直链；linktype=page 时 link 指详情页）",
            "torznab_caps": "/api/v1/torznab",
            "torznab_search": "/api/v1/torznab/search?apikey={token}&t=search|tvsearch|movie&q=",
            "open_recent": "/api/v1/open/recent?limit=50",
            "open_announces": "/api/v1/open/announces?since_id={id}（增量新种流，autobrr 类轮询用）",
            "open_upload": "POST /api/v1/open/torrents（multipart: file=.torrent + 元数据查询参数；按 info_hash 幂等）",
            "aliases": {
                "getrss": "/api/v1/compat/nexusphp/getrss.php?passkey={passkey}",
                "takelogin": "POST /api/v1/compat/nexusphp/takelogin.php（表单）",
                "userdetails": "/api/v1/compat/nexusphp/userdetails.php?passkey={passkey}",
                "details": "/api/v1/compat/nexusphp/details.php?id={id}"
            }
        },
        "rate_limit": {
            "token_default_per_min": 60,
            "headers": "X-RateLimit-Limit / X-RateLimit-Remaining / X-RateLimit-Reset",
            "on_limit": "429 + code 1015 + Retry-After: 60"
        },
        "compatibility_notes": "字段口径对齐 NexusPHP：seeders/leechers/completed/size(字节)/added(Unix 秒)/category。\
    列表同时提供 category_name / category_np / category_newznab 与 info_hash / pieces_hash；\
    对外列表默认**包含零做种新种**（alive=1 可只要活种）。\
    破坏性变更承诺：旧端点保留 ≥2 个版本周期，先发 deprecation 公告（YemaPT 硬删接口为反面教材）。"
    })))
}

pub fn mount_compat(scope: actix_web::Scope) -> actix_web::Scope {
    let scope = scope.service(compat_meta);
    let scope = scope
        .service(nexusphp::compat_np_user)
        .service(nexusphp::compat_np_torrents)
        .service(nexusphp::compat_np_torrent_detail)
        .service(nexusphp::compat_np_download)
        .service(ptpp::ptpp_user_info)
        .service(download_keys::download_key_issue)
        .service(download_keys::download_key_fetch);
    // NP 别名薄壳（0267）：只认硬编码路径的老脚本走这里
    scope
        .service(aliases::alias_getrss)
        .service(aliases::alias_takelogin)
        .service(aliases::alias_userdetails)
        .service(aliases::alias_details)
}

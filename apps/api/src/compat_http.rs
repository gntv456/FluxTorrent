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

mod common;
mod download_keys;
mod nexusphp;
mod ptpp;

use actix_web::{get, HttpResponse};

use crate::dto::ok;
use crate::errors::DomainResult;

/// 架构自描述：第三方适配器（ptool / PT-Plugin-Plus / 脚本作者）的接入入口。
/// 无需鉴权 —— 只包含协议形状，不含任何用户数据。
#[get("/compat/meta")]
async fn compat_meta() -> DomainResult<HttpResponse> {
    Ok(ok(serde_json::json!({
        "name": "FluxTorrent",
        "architecture": "fluxtorrent",
        "compat": { "nexusphp": true },
        "api_root": "/api/v1",
        "auth": {
            "token": "Authorization: Token <fxo_...>（网页端「我的 → API Token」签发，180 天有效）",
            "passkey": "download.php 兼容端点使用 passkey 查询参数（与 NexusPHP 工具习惯一致）"
        },
        "endpoints": {
            "user": "/api/v1/compat/nexusphp/user.json",
            "torrents": "/api/v1/compat/nexusphp/torrents.json?page=1",
            "torrent_detail": "/api/v1/compat/nexusphp/torrent/{id}.json",
            "download": "/api/v1/compat/nexusphp/download.php?id={id}&passkey={passkey}",
            "download_key": "POST /api/v1/downloads/keys {torrent_id} → 30 分钟临时凭证",
            "ptpp_user_info": "/api/v1/plugins/ptppUserInfo（PT-Plugin-Plus 字段口径聚合端点）"
        },
        "compatibility_notes": "字段口径对齐 NexusPHP：seeders/leechers/completed/size(字节)/added(Unix 秒)/category。\
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
    scope
}

//! 兼容层/Torznab/RSS 域的 utoipa 聚合（F3/W-49，frame-plan §3 步④）。
//!
//! 与 open_api.rs 同口径：路径 + 方法 + 描述 + 错误码，schema 从简。
//! 聚合的 14 条 = spec_np.rs 原手写的全部路径（NP 8 条 + ptpp +
//! download keys 3 条 + torznab 2 条 + rss 1 条，去重 /me/tokens 类的
//! 同键多方法 —— 本域无双方法键，恰 14 条）。

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "FluxTorrent Compat API（兼容层/Torznab/RSS 路径表）",
        description = "由 handler 上的 #[utoipa::path] 注解聚合生成"),
    paths(
        crate::compat_http::nexusphp::compat_np_user,
        crate::compat_http::nexusphp::compat_np_torrents,
        crate::compat_http::nexusphp::compat_np_torrent_detail,
        crate::compat_http::nexusphp::compat_np_download,
        crate::compat_http::aliases::alias_getrss,
        crate::compat_http::aliases::alias_takelogin,
        crate::compat_http::aliases::alias_userdetails,
        crate::compat_http::aliases::alias_details,
        crate::compat_http::ptpp::ptpp_user_info,
        crate::compat_http::download_keys::download_key_issue,
        crate::compat_http::download_keys::download_key_revoke,
        crate::compat_http::download_keys::download_key_fetch,
        crate::economy_http::recon::torznab_caps,
        crate::economy_http::recon::torznab_search,
        crate::rss_http::rss_feed,
        // rss-tokens 域（FRAME §7 第 8 条分域起步）：B 批 0352 端点，
        // 前端 rss-tokens.tsx 的接口类型自此由本注解生成替换手写
        crate::rss_http::tokens::rss_token_list,
        crate::rss_http::tokens::rss_token_create,
        crate::rss_http::tokens::rss_token_rotate,
        crate::rss_http::tokens::rss_token_revoke,
    ),
    tags(
        (name = "兼容层(NP)", description = "NexusPHP 口径 JSON / 别名 / 下载凭证"),
        (name = "Torznab", description = "Torznab caps/search（XML）"),
        (name = "RSS", description = "个性化 RSS 订阅（passkey 路径鉴权）"),
        (name = "RSS Token", description = "/rss/tokens：订阅凭证管理（会话鉴权）"),
    )
)]
pub(crate) struct OpenApiCompat;

/// 取本域路径表（serde_json 值），供 spec.rs::build() 合并。
pub(crate) fn paths() -> serde_json::Value {
    let doc = OpenApiCompat::openapi();
    serde_json::to_value(doc.paths).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    /// 路径集合锁定：注解被误删/改路径时此处先红（openapi_cov_guard 再兜底）。
    #[test]
    fn compat_domain_covers_13_path_keys() {
        let paths = super::paths();
        let obj = paths.as_object().expect("utoipa 聚合产物应为 paths 对象");
        let expected = [
            "/api/v1/compat/nexusphp/user.json",
            "/api/v1/compat/nexusphp/torrents.json",
            "/api/v1/compat/nexusphp/torrent/{id}.json",
            "/api/v1/compat/nexusphp/download.php",
            "/api/v1/compat/nexusphp/getrss.php",
            "/api/v1/compat/nexusphp/takelogin.php",
            "/api/v1/compat/nexusphp/userdetails.php",
            "/api/v1/compat/nexusphp/details.php",
            "/api/v1/plugins/ptppUserInfo",
            "/api/v1/downloads/keys",
            "/api/v1/downloads/keys/revoke",
            "/api/v1/downloads/{torrent_id}",
            "/api/v1/torznab",
            "/api/v1/torznab/search",
            "/api/v1/rss/{passkey}",
            // rss-tokens 域（FRAME §7 第 8 条分域起步，0352 端点注解化）
            "/api/v1/rss/tokens",
            "/api/v1/rss/tokens/{id}/rotate",
            "/api/v1/rss/tokens/{id}",
        ];
        let mut miss = Vec::new();
        for p in expected {
            if !obj.contains_key(p) {
                miss.push(p);
            }
        }
        assert!(
            miss.is_empty(),
            "compat 域注解缺路径: {miss:?}（实际键: {:?}）",
            obj.keys().collect::<Vec<_>>()
        );
        // 18 键 19 操作：原 15 单方法键 + rss-tokens 4 操作 3 键
        // （/rss/tokens 双方法 get+post，rotate/revoke 各一键一方法）
        let ops: usize = obj
            .values()
            .map(|v| v.as_object().map(|o| o.len()).unwrap_or(0))
            .sum();
        assert_eq!(ops, 19, "compat 域（含 rss-tokens）应有 19 个操作");
    }
}

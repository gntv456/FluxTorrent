//! 开放 API 域的 utoipa 聚合（F3/W-49，frame-plan §3 步③）。
//!
//! 本批目标口径：「路径 + 方法 + 一句描述 + 错误码列表」机器可读，
//! schema 精度从简（responses 全部走描述文本，不生成 JSON Schema）。
//! components 空起步；既有 spec_schemas.rs 的手写 schema 仍由
//! spec.rs::build() 合并进最终 openapi.json（双轨期：路径来自注解，
//! schema 来自手写，路径段的手写 spec 已归零）。
//!
//! 聚合的 17 条 = mount_openapi 挂载的全部端点（openapi.json 自身除外，
//! 它是文档端点不是数据契约；M36 起含 crossseed 三端点）。

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "FluxTorrent Open API（开放数据域路径表）",
        description = "由 handler 上的 #[utoipa::path] 注解聚合生成"),
    paths(
        crate::openapi_http::open::open_recent_torrents,
        crate::openapi_http::open::open_announces,
        crate::openapi_http::open::open_categories,
        crate::openapi_http::detail_open::open_torrent_detail,
        crate::openapi_http::me::me_overview,
        crate::openapi_http::me::me_hr,
        crate::openapi_http::me::me_messages,
        crate::openapi_http::me_torrents::me_seeding,
        crate::openapi_http::me_torrents::me_history,
        crate::openapi_http::upload::open_upload,
        crate::openapi_http::tokens::token_list,
        crate::openapi_http::tokens::token_issue,
        crate::openapi_http::tokens::token_revoke,
        crate::openapi_http::tokens::token_refresh,
        crate::openapi_http::crossseed::xseed_lookup,
        crate::openapi_http::crossseed::xseed_dl,
        crate::openapi_http::crossseed::xseed_iyuu,
    ),
    tags(
        (name = "开放数据", description = "/open/*：过审种子的只读开放契约"),
        (name = "发种", description = "Token 化发种（需 upload scope）"),
        (name = "我的数据", description = "/open/me/*：Token 主人自己的数据"),
        (name = "凭据", description = "/me/tokens：API Token 管理面（会话鉴权）"),
        (name = "辅种(crossseed)",
            description = "/crossseed/*：辅种指纹查询与下载（需 crossseed scope）"),
    )
)]
pub(super) struct OpenApiOpen;

/// 取本域路径表（serde_json 值），供 spec.rs::build() 合并。
pub(super) fn paths() -> serde_json::Value {
    let doc = OpenApiOpen::openapi();
    serde_json::to_value(doc.paths).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    /// 路径集合锁定：注解被误删/改路径时此处先红（openapi_cov_guard 再兜底）。
    /// M36：crossseed 三端点（lookup / dl / iyuu）入列。
    #[test]
    fn open_api_domain_covers_17_paths() {
        let paths = super::paths();
        let obj = paths.as_object().expect("utoipa 聚合产物应为 paths 对象");
        let expected = [
            "/api/v1/open/recent",
            "/api/v1/open/announces",
            "/api/v1/open/categories",
            "/api/v1/open/torrents",
            "/api/v1/open/torrents/{id}",
            "/api/v1/open/me/overview",
            "/api/v1/open/me/seeding",
            "/api/v1/open/me/history",
            "/api/v1/open/me/hr",
            "/api/v1/open/me/messages",
            "/api/v1/me/tokens",
            "/api/v1/me/tokens/revoke",
            "/api/v1/me/tokens/refresh",
            "/api/v1/crossseed/lookup",
            "/api/v1/crossseed/dl/{torrent_id}",
            "/api/v1/crossseed/iyuu",
        ];
        let mut miss = Vec::new();
        for p in expected {
            if !obj.contains_key(p) {
                miss.push(p);
            }
        }
        // /me/tokens 上有两个 method（get+post），16 个 path 键 = 17 个操作
        assert!(
            miss.is_empty(),
            "开放 API 域注解缺路径: {miss:?}（实际键: {:?}）",
            obj.keys().collect::<Vec<_>>()
        );
        let ops: usize = obj
            .values()
            .map(|v| v.as_object().map(|o| o.len()).unwrap_or(0))
            .sum();
        assert_eq!(
            ops, 17,
            "开放 API 域应有 17 个操作（16 键 + /me/tokens 双方法）"
        );
    }
}

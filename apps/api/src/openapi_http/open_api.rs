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
        crate::torrent_http::torrent_aggregate,
        crate::torrent_http::batch_download,
        crate::torrent_http::cue_check,
        crate::torrent_http::detail,
        crate::torrent_http::torrent_detail_ext,
        crate::torrent_http::torrent_files,
        crate::torrent_http::torrent_thanks,
        crate::torrent_http::comments,
        crate::torrent_http::torrent_snatches,
        crate::torrent_http::torrent_nfo,
        crate::torrent_http::file_conflicts,
        crate::torrent_http::create_comment,
        crate::torrent_http::delete_comment,
        crate::torrent_http::toggle_comment_like,
        crate::torrent_http::do_thank,
        crate::torrent_http::request_reseed,
        crate::torrent_http::torrent_tags,
        crate::torrent_http::torrent_tag_put,
        crate::torrent_http::do_bookmark,
        crate::torrent_http::list,
        crate::torrent_http::torrent_log_body,
        crate::torrent_http::log_adjust,
        crate::torrent_http::torrent_magnet,
        crate::torrent_http::edit_torrent,
        crate::torrent_http::set_torrent_price,
        crate::torrent_http::restore_torrent,
        crate::torrent_http::resubmit_torrent,
        crate::torrent_http::delete_torrent,
        crate::torrent_http::torrent_peers,
        crate::torrent_http::preview_add,
        crate::torrent_http::preview_list,
        crate::torrent_http::preview_remove,
        crate::torrent_http::torrent_related,
        crate::torrent_http::torrent_suggest,
        crate::torrent_http::torrent_trump,
        crate::torrent_http::torrent_trumps,
        crate::torrent_http::trump_list,
        crate::torrent_http::trump_resolve,
    ),
    tags(
        (name = "开放数据", description = "/open/*：过审种子的只读开放契约"),
        (name = "发种", description = "Token 化发种（需 upload scope）"),
        (name = "我的数据", description = "/open/me/*：Token 主人自己的数据"),
        (name = "凭据", description = "/me/tokens：API Token 管理面（会话鉴权）"),
        (name = "种子",
            description = "/torrents/*：列表/详情/互动/管理/磁力（会话鉴权域）"),
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
    /// F4（2026-10-16）：种子域 38 条路由注解化后并入本聚合，
    /// 合计 17 + 38 = **55 个操作**。
    #[test]
    fn open_api_domain_covers_55_paths() {
        let paths = super::paths();
        let obj = paths.as_object().expect("utoipa 聚合产物应为 paths 对象");
        let expected = [
            // 开放 API 域（原有 17）
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
            // 种子域（F4 新增 38 条的路径键，抽锚点）
            "/api/v1/torrents",
            "/api/v1/torrents/{id}",
            "/api/v1/torrents/{id}/detail",
            "/api/v1/torrents/{id}/files",
            "/api/v1/torrents/{id}/thanks",
            "/api/v1/torrents/{id}/comments",
            "/api/v1/torrents/{id}/snatches",
            "/api/v1/torrents/{id}/nfo",
            "/api/v1/torrents/{id}/magnet",
            "/api/v1/torrents/{id}/peers",
            "/api/v1/torrents/{id}/tags",
            "/api/v1/torrents/{id}/related",
            "/api/v1/torrents/{id}/aggregate",
            "/api/v1/torrents/batch-download",
            "/api/v1/torrents/suggest",
            "/api/v1/admin/trumps",
        ];
        let mut miss = Vec::new();
        for p in expected {
            if !obj.contains_key(p) {
                miss.push(p);
            }
        }
        assert!(
            miss.is_empty(),
            "聚合路径缺: {miss:?}（实际键: {:?}）",
            obj.keys().collect::<Vec<_>>()
        );
        let ops: usize = obj
            .values()
            .map(|v| v.as_object().map(|o| o.len()).unwrap_or(0))
            .sum();
        // 开放域 17 操作（16 键，/me/tokens 双方法）+ 种子域 38 操作
        // （/torrents/{id} 上有 get/put/delete 三方法、/torrents/{id}/tags
        // 有 get/put、/torrents/{id}/comments 有 get/post、
        // /torrents/{id}/previews 有 get/post/delete，故键数少于操作数）
        assert_eq!(
            ops, 55,
            "聚合应有 55 个操作（开放 17 + 种子 38）"
        );
    }
}

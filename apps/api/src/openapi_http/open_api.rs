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
        crate::torrent_http::delete_preview,
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
        // F4（2026-10-16）社区域 85 条路由注解化，
        // 聚合合计 17 开放 + 38 种子 + 85 社区 = 140 个操作。
        crate::community_http::admin_match_create,
        crate::community_http::admin_match_score,
        crate::community_http::artists_list,
        crate::community_http::artists_top,
        crate::community_http::artist_detail,
        crate::community_http::contact_staff,
        crate::community_http::follow_create,
        crate::community_http::follow_mine,
        crate::community_http::follow_status,
        crate::community_http::follow_delete,
        crate::community_http::forum_list,
        crate::community_http::forum_boards,
        crate::community_http::bounty_award,
        crate::community_http::forum_embed_rules,
        crate::community_http::forum_feed,
        crate::community_http::lottery_draw,
        crate::community_http::lottery_fairness,
        crate::community_http::lottery_join,
        crate::community_http::poll_close,
        crate::community_http::poll_vote,
        crate::community_http::post_delete,
        crate::community_http::post_edit,
        crate::community_http::post_like,
        crate::community_http::post_unlike,
        crate::community_http::forum_search,
        crate::community_http::forum_tags_dict,
        crate::community_http::post_tip,
        crate::community_http::topic_create,
        crate::community_http::topic_manage_batch,
        crate::community_http::topic_delete,
        crate::community_http::topic_detail,
        crate::community_http::topic_favorite,
        crate::community_http::topic_unfavorite,
        crate::community_http::topic_manage,
        crate::community_http::post_reply,
        crate::community_http::topic_list,
        crate::community_http::friend_add,
        crate::community_http::friend_list,
        crate::community_http::friend_action,
        crate::community_http::matches_list,
        crate::community_http::match_detail,
        crate::community_http::list_prefs_get,
        crate::community_http::list_prefs_set,
        crate::community_http::my_medals,
        crate::community_http::message_stream,
        crate::community_http::notice_prefs_get,
        crate::community_http::notice_prefs_set,
        crate::community_http::my_staff_messages,
        crate::community_http::my_ticket_confirm,
        crate::community_http::my_subscription_calendar,
        crate::community_http::my_group_subscriptions,
        crate::community_http::my_network_subscriptions,
        crate::community_http::medal_rarities,
        crate::community_http::medal_list,
        crate::community_http::medal_buy,
        crate::community_http::my_medal_progress,
        crate::community_http::medal_gift,
        crate::community_http::medal_wear,
        crate::community_http::message_send,
        crate::community_http::message_box_upsert,
        crate::community_http::message_boxes,
        crate::community_http::message_delete,
        crate::community_http::message_inbox,
        crate::community_http::message_markread,
        crate::community_http::message_move,
        crate::community_http::message_sent,
        crate::community_http::message_staff,
        crate::community_http::networks_list,
        crate::community_http::networks_top,
        crate::community_http::network_detail,
        crate::community_http::network_subscribe,
        crate::community_http::network_unsubscribe,
        crate::community_http::pool_honor,
        crate::community_http::seasonal_anime,
        crate::community_http::shoutbox_list,
        crate::community_http::shoutbox_send,
        crate::community_http::shoutbox_bot_help,
        crate::community_http::shoutbox_bot_exec,
        crate::community_http::shoutbox_delete,
        crate::community_http::leak_list,
        crate::community_http::leak_resolve,
        crate::community_http::staff_messages,
        crate::community_http::staff_answer,
        crate::community_http::staff_delete,
        crate::community_http::staff_mark,
        crate::community_http::ticket_list,
        crate::community_http::ticket_update,
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
        (name = "社区/论坛", description = "/forums/*：版块/主题/帖子读写与互动"),
        (name = "社区/消息", description = "/messages/*：站内私信收发、信件箱与流式推送"),
        (name = "社区/联络",
            description = "/shoutbox/*、/leaks/*、/bounty/*：喊话/举报/悬赏"),
        (name = "社区/勋章", description = "/medals/*：勋章列表、佩戴、购买与赠送"),
        (name = "社区/关注", description = "/follows/*：关注对象的状态与我的关注列表"),
        (name = "社区/好友", description = "/friends/*：好友申请、列表与同意/拒绝"),
        (name = "社区/通知", description = "/notice/*、/pool/honor：通知偏好与声望池"),
        (name = "社区/订阅", description = "/subscriptions/*、/calendar：机组/网络/订阅日历"),
        (name = "社区/网络", description = "/networks/*：网络（内网组）列表与详情"),
        (name = "社区/艺人", description = "/artists/*：艺人信息库"),
        (name = "社区/赛事", description = "/matches/*：赛事列表与详情"),
        (name = "社区/站务", description = "/staffbox/*：客服工单与私信"),
        (name = "社区/社区", description = "/lists/prefs、/poll/*：列表偏好与投票"),
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
    /// F4（2026-10-16）：种子域 38 条 + 社区域 85 条路由注解化后并入本聚合，
    /// 合计 17 + 38 + 87 = **142 个操作**。
    #[test]
    fn open_api_domain_covers_142_paths() {
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
            // 社区域（F4 新增 85 条的路径键，抽锚点）
            "/api/v1/forums",
            "/api/v1/forums/search",
            "/api/v1/forums/{id}/topics",
            "/api/v1/forums/topics/{id}",
            "/api/v1/forums/lottery/{topic_id}/fairness",
            "/api/v1/messages/inbox",
            "/api/v1/messages/boxes",
            "/api/v1/me/messages/stream",
            "/api/v1/medals",
            "/api/v1/medals/progress",
            "/api/v1/me/medals",
            "/api/v1/networks",
            "/api/v1/artists",
            "/api/v1/shoutbox",
            "/api/v1/matches",
            "/api/v1/friends",
            "/api/v1/follows/mine",
            "/api/v1/stafftickets",
            "/api/v1/staffmessages",
            "/api/v1/seasonal",
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
        // + 社区域 85 操作（76 键，同键多方法：like/unlike、
        // post_like 与 post_unlike 分列 get/post 风格的 PUT/DELETE）
        // （/torrents/{id} 上有 get/put/delete 三方法、/torrents/{id}/tags
        // 有 get/put、/torrents/{id}/comments 有 get/post、
        // /torrents/{id}/previews 有 get/post/delete，故键数少于操作数）
        assert_eq!(
            ops, 142,
            "聚合应有 142 个操作（开放 17 + 种子 38 + 社区 87）"
        );
    }
}

//! 社区模块 HTTP 接口（M14 勋章 + M15 论坛 + M16 短讯/好友）。
//! 按域拆分（300 行门禁）：见各子模块头注释。

use actix_web::web;
mod bounty;
mod embed_rules;
mod feed;
mod follow;
mod forum_perm;
mod forum_util;
mod forums;
mod friend;
mod interact;
mod leak_bot;
mod like;
mod lottery;
pub mod medal;
mod medal_buy;
mod medal_gift;
mod message;
mod message_box;
mod message_send;
mod notice;
mod post_ctx;
mod post_edit;
mod post_group;
mod post_read;
mod post_write;
mod shoutbox;
mod staff_answer;
mod staffbox;
mod ticket;
mod tip;
mod topic;
mod topic_admin;
mod topic_create;

pub use bounty::*;
pub use embed_rules::*;
pub use feed::*;
pub use follow::*;
pub use forum_perm::*;
pub use forum_util::*;
pub use forums::*;
pub use friend::*;
pub use interact::*;
pub use leak_bot::*;
pub use like::*;
pub use lottery::*;
pub use medal::*;
pub use medal_buy::*;
pub use medal_gift::*;
pub use message::*;
pub use message_box::*;
pub use message_send::*;
pub use notice::*;
pub use post_ctx::*;
pub use post_edit::*;
pub use post_read::*;
pub use post_write::*;
pub use shoutbox::*;
pub use staff_answer::*;
pub use staffbox::*;
pub use ticket::*;
pub use tip::*;
pub use topic::*;
pub use topic_admin::*;
pub use topic_create::*;

pub fn mount_community(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M14 勋章
        .service(medal_rarities)
        .service(medal_list)
        .service(medal_buy)
        .service(medal_gift)
        .service(medal_wear)
        .service(my_medals)
        .service(notice_prefs_get)
        .service(notice_prefs_set)
        .service(pool_honor)
        // M15 论坛
        .service(forum_list)
        .service(forum_boards)
        .service(forum_search)
        .service(topic_create)
        .service(topic_list)
        .service(topic_detail)
        .service(post_reply)
        .service(post_edit)
        .service(post_delete)
        .service(topic_delete)
        .service(topic_manage)
        .service(topic_manage_batch)
        // M15 论坛互动（0116 点赞/收藏）
        .service(post_like)
        .service(post_unlike)
        .service(topic_favorite)
        .service(topic_unfavorite)
        // M15 论坛关注订阅（0121 用户/版块/主题 + 关注流）
        .service(follow_create)
        .service(follow_delete)
        .service(follow_status)
        .service(follow_mine)
        .service(forum_feed)
        // M15 论坛标签（0123：词表复用 tag_dict，只建 topic_tags 关联）
        .service(forum_tags_dict)
        // 论坛视频内嵌 V1（0189）：白名单 embed 规则只读端点
        .service(forum_embed_rules)
        // M15 论坛悬赏（0124：发帖冻结 → 楼主采纳发放，复用求种悬赏范式）
        .service(bounty_award)
        // M15 论坛投票（0125：发帖定选项 → 一人一票 → 楼主可截止，范式照 fun_polls）
        .service(poll_vote)
        .service(poll_close)
        // M15 论坛抽奖（0126：发帖冻结奖金池 → 付费/免费参与 → 到点或手动开奖，jgg 经济范式）
        .service(lottery_join)
        .service(lottery_draw)
        // M15 论坛打赏（0127：楼层打赏 = spend/earn 同额对冲，不抽税）
        .service(post_tip)
        // M16 短讯与好友
        .service(message_send)
        .service(message_markread)
        .service(message_delete)
        .service(message_move)
        .service(message_boxes)
        .service(message_box_upsert)
        .service(contact_staff)
        .service(staff_messages)
        .service(my_staff_messages)
        .service(my_ticket_confirm)
        .service(staff_answer)
        .service(staff_mark)
        .service(staff_delete)
        .service(shoutbox_list)
        .service(shoutbox_send)
        .service(shoutbox_delete)
        .service(shoutbox_bot_help)
        .service(shoutbox_bot_exec)
        .service(ticket_list)
        .service(ticket_update)
        .service(leak_list)
        .service(leak_resolve)
        .service(message_inbox)
        .service(message_sent)
        .service(message_staff)
        .service(friend_add)
        .service(friend_list)
        .service(friend_action)
}

/// 主配置：主 scope + 安装向导 + 经济路由 + 社区路由（单一 /api/v1 scope）
pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = crate::setup_http::mount_setup(
        crate::economy_http::mount_economy(crate::http::v1_scope()),
    );
    let scope = mount_community(scope);
    let scope = crate::games_http::mount_games(crate::ops_http::mount_ops(
        crate::content_http::mount_content(scope),
    ));
    let scope = crate::admin_p3_http::mount_p3_tools(
        crate::admin_p2_http::mount_p2_tools(crate::admin_http::mount_admin(
            scope,
        )),
    );
    let scope = crate::settings_http::mount_settings(scope);
    let scope = crate::adapter_http::mount_adapters(scope);
    let scope = crate::push_http::mount_push(scope);
    let scope = crate::gaps_http::mount_gaps(scope);
    let scope = crate::rss_http::mount_rss(scope);
    let scope = crate::twofa_http::mount_twofa(scope);
    let scope = crate::compat_http::mount_compat(scope);
    let scope = crate::v4_http::mount_v4(scope);
    cfg.service(crate::openapi_http::mount_openapi(scope));
}

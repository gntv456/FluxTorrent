//! 社区模块 HTTP 接口（M14 勋章 + M15 论坛 + M16 短讯/好友）。
//! 按域拆分（300 行门禁）：见各子模块头注释。

use actix_web::web;
mod artists;
mod bounty;
mod embed_rules;
mod embed_video;
mod feed;
mod follow;
mod forum_perm;
mod forum_util;
mod forums;
mod friend;
mod interact;
mod leak_bot;
mod like;
mod list_prefs;
mod lottery;
pub mod medal;
mod medal_buy;
mod medal_gift;
mod message;
mod message_box;
mod message_folders;
mod message_send;
mod message_stream;
mod network_subs;
mod networks;
mod notice;
mod post_ctx;
mod post_edit;
mod post_group;
mod post_read;
mod post_write;
mod seasonal;
mod shoutbox;
mod sport_matches;
mod staff_answer;
mod staffbox;
mod subs_calendar;
mod ticket;
mod tip;
mod topic;
mod topic_admin;
mod topic_create;

pub use artists::*;
pub use bounty::*;
pub use embed_rules::*;
pub use embed_video::*;
pub use feed::*;
pub use follow::*;
pub use forum_perm::*;
pub use forum_util::*;
pub use forums::*;
pub use friend::*;
pub use interact::*;
pub use leak_bot::*;
pub use like::*;
pub use list_prefs::*;
pub use lottery::*;
pub use medal::*;
pub use medal_buy::*;
pub use medal_gift::*;
pub use message::*;
pub use message_box::*;
pub use message_folders::*;
pub use message_send::*;
pub use message_stream::*;
pub use network_subs::*;
pub use networks::*;
pub use notice::*;
pub use post_ctx::*;
pub use post_edit::*;
pub use post_read::*;
pub use post_write::*;
pub use seasonal::*;
pub use shoutbox::*;
pub use sport_matches::*;
pub use staff_answer::*;
pub use staffbox::*;
pub use subs_calendar::*;
pub use ticket::*;
pub use tip::*;
pub use topic::*;
pub use topic_admin::*;
pub use topic_create::*;

pub fn mount_community(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // 艺人实体（0327 music）：列表/艺人页/榜单
        .service(artists_list)
        .service(artists_top)
        .service(artist_detail)
        // 出品方聚合页（0330 documentary 专项 P1）：厂牌列表/厂牌页/榜单
        .service(networks_list)
        .service(networks_top)
        .service(network_detail)
        // 厂牌订阅（0332 收官）：订新片 + 我的订阅
        .service(network_subscribe)
        .service(network_unsubscribe)
        .service(my_network_subscriptions)
        // 追更中心（0328 剧集批）：我的订阅进度 + 日历
        .service(my_group_subscriptions)
        .service(my_subscription_calendar)
        // 当季新番（0334 anime 批）：按播出季浏览
        .service(seasonal_anime)
        // 体育对阵（0330 sports 批）
        .service(matches_list)
        .service(match_detail)
        .service(admin_match_create)
        .service(admin_match_score)
        // M14 勋章
        .service(medal_rarities)
        .service(medal_list)
        .service(medal_buy)
        .service(medal_gift)
        .service(medal_wear)
        .service(my_medals)
        .service(notice_prefs_get)
        .service(notice_prefs_set)
        .service(list_prefs_get)
        .service(list_prefs_set)
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
        // 未读数 SSE 流（EventSource 走 Cookie 鉴权，首帧即推 + 15s 轮询）
        .service(message_stream)
        .service(friend_add)
        .service(friend_list)
        .service(friend_action)
}

/// 主配置：主 scope + 安装向导 + 经济路由 + 社区路由（单一 /api/v1 scope）
pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = crate::setup_http::mount_setup(
        crate::economy_http::mount_economy(crate::http::v1_scope()),
    );
    // CSRF token 签发口（独立于登录，供前端按需续期/轮换）。
    // 挂在这里而不是在 App 上另开一个 /api/v1 scope：actix 的 scope
    // 是前缀独占匹配，另开一个会把原有 700+ 路由全打成 404。

    let scope = mount_community(scope);
    let scope = crate::games_http::mount_games(crate::ops_http::mount_ops(
        crate::content_http::mount_content(scope),
    ));
    let scope = crate::gacha_http::mount_gacha(scope);
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
    let scope = crate::openapi_http::mount_openapi(scope);
    // CSRF token 签发口挂在同一个 /api/v1 scope（链尾）。
    // 位置有讲究：**不要**在 App 上另开 /api/v1 scope（actix 的 scope
    // 是前缀独占匹配，另开会把原有 700+ 路由全打成 404——本批踩过）；
    // 也不要插在链中间（同样会让先注册的服务整批失效，只有最后
    // 配的那个能通）。链尾是最稳的位置：后续 mount_* 都接在它后面。
    let scope = scope.configure(crate::csrf::configure);
    cfg.service(scope);
}

#[cfg(test)]
mod scope_tree_tests {
    use actix_web::{test, web, App};

    /// 真实装配链的冒烟：configure 插在链尾之后，
    /// 链上其它域的路由是否还活着。
    ///
    /// 本批踩过的坑就是「只有 csrf 通、其余全 404」——
    /// 编译、单测、路由清单三样都绿，只有真打一次才看得见。
    #[actix_web::test]
    async fn csrf_scope_does_not_eat_the_chain() {
        let app = test::init_service(App::new().configure(super::configure))
            .await;
        for p in [
            "/api/v1/ready",
            "/api/v1/health",
            "/api/v1/setup/status",
            "/api/v1/me/csrf",
        ] {
            let r = test::call_service(
                &app,
                test::TestRequest::get().uri(p).to_request(),
            )
            .await;
            assert_ne!(
                r.status().as_u16(),
                404,
                "{} 404 —— csrf 的 configure 把整条链吃掉了",
                p
            );
        }
    }
}
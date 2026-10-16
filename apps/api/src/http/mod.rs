//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。
//! 按域拆分（300 行门禁）：鉴权/限流设施在 auth_infra.rs，me 杂项/统计/
//! 举报/RSS 在 misc_handlers.rs，插件四件套在 plugins.rs，公告/趣味盒/
//! 友链在 news_fun.rs，首页板块在 home.rs；v1_scope 路由注册留在此，
//! auth 经 state.jwt 使用（0071 RS256 化后本层不再直接调用）。

pub(crate) mod write_origin;
pub(crate) use write_origin::write_origin_mw;
mod auth_infra;
mod cheat_case;
mod cheat_http;
mod frames_gomoku;
mod fun_links;
mod home;
mod home_attendance;
mod home_layout;
mod home_news;
pub mod idempotency;
mod links;
mod misc_handlers;
mod news_fun;
mod plugins;
mod probes;
mod qenv;
mod reports_http;
mod risk;
mod throttle;
pub mod view_layout;

pub use auth_infra::*;
pub use cheat_case::*;
pub use cheat_http::*;
pub use frames_gomoku::*;
pub use fun_links::*;
pub use home::*;
pub use home_layout::*;
pub use links::*;
pub use misc_handlers::*;
pub use news_fun::*;
pub use plugins::*;
pub use probes::*;
pub use qenv::*;
pub use risk::*;
pub use throttle::*;
pub use view_layout::*;

/// 出网抓取统一超时（更新检查/商店索引/远程包/PT-Gen/适配器拉取共用）：
/// 原 8s 与 20s 两档并存，统一为 20（P2 收敛）——只影响失败得更慢。
pub(crate) const OUTBOUND_FETCH_TIMEOUT_SECS: u64 = 20;

use crate::social_http::{
    endangered_list, team_create, team_join, team_leave, team_list, team_mine,
};
use actix_web::web;

pub fn v1_scope() -> actix_web::Scope {
    let scope = web::scope("/api/v1")
        .service(health)
        .service(ready)
        .service(about)
        .service(crate::auth_http::register)
        .service(crate::auth_http::email_policy_endpoint)
        .service(crate::auth_http::register_fields_endpoint)
        .service(crate::auth_http::register_mode_endpoint)
        .service(crate::staff_http::custom_page_public)
        .service(crate::staff_http::custom_pages_public_list)
        .service(crate::staff_http::help_docs_list)
        .service(crate::staff_http::terms_list)
        .service(crate::staff_http::terms_upsert)
        .service(crate::staff_http::terms_toggle)
        .service(crate::staff_http::terms_delete)
        // 术语治理后台面（0386）：键登记 / 状态迁移 / 版本 /
        // 巡检五条端点。此前只写了处理函数（模块也已 export），但
        // **从未注册到路由表**——实测三条读口全回 `1004 接口不存在`，
        // 写口同样。产品上线了、迁移 0386 也入库了，却没有一条可调用的接口。
        .service(crate::staff_http::keys_list)
        .service(crate::staff_http::key_upsert)
        .service(crate::staff_http::key_set_status)
        .service(crate::staff_http::versions_list)
        .service(crate::staff_http::version_restore)
        .service(crate::staff_http::run_audit)
        // 运维 webhook 管理面（0389）：出网白名单 / 出口健康度 / 投递台账。
        .service(crate::admin_http::list)
        .service(crate::admin_http::add_cidr)
        .service(crate::admin_http::del_cidr)
        .service(crate::admin_http::reset)
        .service(crate::admin_http::deliveries)
        // 反作弊规则集版本化 + 影子 + 回测（0391）：
        // 改权重前先存草稿 → 影子跑一轮看会多判谁 → 回测历史 → 才切换。
        .service(crate::admin_http::list_rulesets)
        .service(crate::admin_http::create_ruleset)
        .service(crate::admin_http::activate)
        .service(crate::admin_http::set_shadow)
        .service(crate::admin_http::shadow_diff)
        .service(crate::admin_http::run_backtest)
        .service(crate::admin_http::backtests)
        .service(crate::staff_http::custom_pages_list)
        .service(crate::staff_http::custom_pages_add)
        .service(crate::staff_http::custom_pages_update)
        .service(crate::staff_http::custom_pages_delete)
        // Wiki（0359）：公开读（网关按 /wiki 前缀挂模块开关）+ 后台 CRUD/版本史
        .service(crate::staff_http::wiki_public_index)
        .service(crate::staff_http::wiki_public_page)
        .service(crate::staff_http::wiki_list)
        .service(crate::staff_http::wiki_create)
        .service(crate::staff_http::wiki_update)
        .service(crate::staff_http::wiki_delete)
        .service(crate::staff_http::wiki_revisions)
        .service(crate::auth_http::login)
        // 邮箱激活通道（C7-#4）：verify 匿名点击（邮件链接）、resend 限流
        .service(crate::auth_http::email_verify)
        .service(crate::auth_http::email_resend)
        // passkey（WebAuthn）第二通道（0227）
        // passkey_list（GET /me/passkeys）在此前只进了 mount_passkey——而该
        // mount 无调用者（逐个挂载时被遗漏），前端 passkeys-card 长期 404。
        .service(crate::auth_http::passkey_list)
        .service(crate::auth_http::passkey_begin)
        .service(crate::auth_http::passkey_finish)
        .service(crate::auth_http::passkey_login_begin)
        .service(crate::auth_http::passkey_login_finish)
        .service(crate::auth_http::passkey_remove)
        .service(crate::auth_http::me)
        .service(crate::auth_http::me_perms)
        .service(crate::auth_http::me_overview)
        .service(crate::auth_http::me_settings_get)
        .service(crate::auth_http::me_settings_put)
        .service(my_torrentlist)
        .service(crate::auth_http::user_torrentlist)
        .service(my_bookmarks)
        .service(crate::auth_http::rotate_passkey)
        .service(crate::auth_http::my_login_history)
        .service(crate::auth_http::my_sessions)
        .service(crate::auth_http::revoke_other_sessions)
        .service(crate::attachment_http::upload_attachment)
        .service(crate::attachment_http::get_attachment)
        .service(crate::attachment_video_upload::upload_video)
        .service(crate::attachment_video::head_attachment)
        .service(crate::auth_http::me_password_change)
        .service(crate::auth_http::me_title_get)
        .service(crate::auth_http::me_title_set)
        .service(crate::auth_http::user_public_profile)
        .service(crate::auth_http::my_fields)
        .service(crate::auth_http::my_fields_put)
        .service(crate::auth_http::user_fields_list)
        .service(crate::auth_http::user_fields_add)
        .service(crate::auth_http::user_fields_update)
        .service(crate::auth_http::user_fields_delete)
        .service(crate::torrent_http::list)
        // suggest 必须先于 detail（/torrents/{id}）注册：actix 按注册顺序
        // 匹配，{id} 的 Path<i64> 解析失败会直接 404 而不是fallthrough
        .service(crate::torrent_http::torrent_suggest)
        .service(crate::torrent_http::detail)
        .service(crate::torrent_http::torrent_aggregate)
        .service(crate::torrent_http::torrent_detail_ext)
        // 抓轨日志正文（0312）：详情页「查看日志」懒加载，四段路径不与
        // /torrents/{id} 冲突
        .service(crate::torrent_http::torrent_log_body)
        .service(crate::torrent_http::torrent_peers)
        .service(crate::torrent_http::torrent_related)
        .service(crate::torrent_http::torrent_magnet)
        .service(crate::torrent_http::batch_download)
        .service(crate::torrent_http::torrent_files)
        .service(crate::torrent_http::torrent_thanks)
        .service(crate::torrent_http::comments)
        .service(crate::torrent_http::create_comment)
        .service(crate::torrent_http::delete_comment)
        .service(crate::torrent_http::toggle_comment_like)
        .service(crate::torrent_http::do_thank)
        .service(crate::torrent_http::do_bookmark)
        .service(crate::torrent_http::edit_torrent)
        .service(crate::torrent_http::set_torrent_price)
        .service(crate::torrent_http::delete_torrent)
        .service(crate::torrent_http::restore_torrent)
        .service(crate::torrent_http::resubmit_torrent)
        .service(crate::torrent_http::torrent_trump)
        .service(crate::torrent_http::torrent_trumps)
        .service(crate::torrent_http::trump_list)
        .service(crate::torrent_http::trump_resolve)
        .service(crate::torrent_http::preview_add)
        .service(crate::torrent_http::preview_list)
        .service(crate::torrent_http::preview_remove)
        .service(crate::torrent_http::log_adjust)
        .service(crate::torrent_http::cue_check)
        .service(crate::torrent_http::file_conflicts)
        .service(crate::publish_http::group_attach)
        .service(crate::publish_http::group_info)
        .service(crate::publish_http::group_subscribe)
        .service(crate::publish_http::collection_create)
        .service(crate::publish_http::collection_add)
        .service(crate::publish_http::collections_list)
        .service(crate::publish_http::collection_detail)
        .service(crate::publish_http::torrent_collections)
        .service(crate::publish_http::group_unsubscribe)
        .service(crate::torrent_http::torrent_snatches)
        .service(crate::torrent_http::torrent_nfo)
        .service(crate::torrent_http::request_reseed)
        .service(crate::torrent_http::torrent_tags)
        .service(crate::torrent_http::torrent_tag_put)
        .service(stats)
        .service(cheat_events_list)
        .service(cheat_events_resolve);
    let scope = cheat_case::mount_cheat_cases(scope);
    let scope = risk::mount_risk(scope);
    scope
        .service(reports_http::report_create)
        .service(rss_info)
        .service(news_create)
        .service(news_update)
        .service(news_delete)
        .service(fun_items)
        .service(fun_item_vote)
        .service(fun_item_create)
        .service(fun_item_update)
        .service(fun_item_set_status)
        .service(fun_item_delete)
        .service(link_apply)
        .service(link_admin_list)
        .service(link_update)
        .service(link_delete)
        .service(crate::staff_http::faq_list)
        .service(crate::staff_http::faq_create)
        .service(crate::staff_http::faq_update)
        .service(crate::staff_http::faq_delete)
        .service(crate::staff_http::rules_content)
        .service(crate::staff_http::rule_create)
        .service(crate::staff_http::rule_update)
        .service(crate::staff_http::rule_delete)
        .service(crate::staff_http::category_list)
        .service(crate::staff_http::category_create)
        .service(crate::staff_http::category_update)
        .service(crate::staff_http::category_delete)
        .service(crate::staff_http::ban_list)
        .service(crate::staff_http::ban_create)
        .service(crate::staff_http::ban_delete)
        .service(crate::staff_http::freeleech_set)
        .service(crate::staff_http::freeleech_clear)
        .service(crate::staff_http::freeleech_update)
        .service(crate::staff_http::freeleech_delete)
        .service(crate::staff_http::freeleech_list)
        .service(crate::staff_http::promo_intent_submit)
        .service(crate::staff_http::promo_intent_list)
        .service(crate::staff_http::promo_intent_approve)
        .service(crate::staff_http::promo_intent_reject)
        .service(crate::staff_http::promo_intent_cancel)
        .service(crate::staff_http::staffmess_send)
        .service(crate::staff_http::admin_add_user)
        .service(crate::staff_http::admin_amount_bonus)
        .service(crate::staff_http::warned_list)
        .service(crate::staff_http::warn_user)
        .service(crate::staff_http::unwarn_user)
        .service(crate::staff_http::ipcheck)
        .service(crate::staff_http::maxlogin)
        .service(crate::staff_http::admin_amount_upload)
        .service(crate::staff_http::admin_reset_pass)
        .service(crate::staff_http::admin_delete_disabled)
        .service(crate::staff_http::emailban_list)
        .service(crate::staff_http::emailban_create)
        .service(crate::staff_http::emailban_delete)
        .service(crate::staff_http::test_ip)
        .service(crate::staff_http::admin_stats)
        .service(crate::staff_http::clear_cache)
        .service(crate::staff_http::seed_stats)
        .service(crate::staff_http::do_cleanup)
        .service(crate::staff_http::ad_list)
        .service(crate::staff_http::ad_create)
        .service(crate::staff_http::ad_update)
        .service(crate::staff_http::ad_toggle)
        .service(crate::staff_http::ad_delete)
        .service(crate::staff_http::not_connectable)
        .service(crate::staff_http::conn_dist)
        .service(crate::staff_http::uploaders)
        .service(crate::staff_http::all_agents)
        .service(crate::staff_http::poll_overview)
        .service(crate::staff_http::db_stats)
        .service(crate::staff_http::sys_log)
        .service(crate::staff_http::locations)
        .service(crate::staff_http::donate_state)
        .service(crate::staff_http::donate_topup)
        .service(crate::staff_http::donate_order_status)
        .service(crate::staff_http::donate_notify)
        .service(crate::staff_http::donate_order)
        .service(crate::staff_http::admin_payment_orders)
        .service(crate::staff_http::admin_payment_complete)
        .service(crate::staff_http::site_profile)
        .service(crate::staff_http::site_type_pack_list)
        .service(crate::staff_http::site_type_pack_apply)
        .service(crate::staff_http::site_type_pack_diff)
        .service(crate::staff_http::site_type_pack_save)
        .service(crate::staff_http::site_pack_applies)
        .service(crate::staff_http::site_pack_rollback)
        .service(crate::staff_http::massmail_list)
        .service(crate::staff_http::massmail_send)
        .service(medal_wall)
        .service(contest_list)
        .service(contest_join)
        .service(frame_list)
        .service(frame_equip)
        .service(gomoku_create)
        .service(gomoku_join)
        .service(gomoku_move)
        .service(gomoku_get)
        .service(home_sections)
        .service(admin_home_layout_put)
        .service(admin_view_hidden_put)
        .service(view_layout_keys)
        .service(crate::publish_http::upload)
        .service(crate::publish_http::ptgen)
        .service(crate::publish_http::auto_meta)
        .service(crate::publish_http::download)
        .service(crate::invite_http::issue_invite_handler)
        .service(crate::invite_http::redeem_invite_handler)
        .service(crate::invite_http::list_invites_handler)
        .service(crate::invite_http::invites_status_handler)
        .service(crate::invite_http::revoke_invite_handler)
        .service(crate::invite_http::list_invitees_handler)
        .service(crate::invite_http::email_invite_handler)
        .service(endangered_list)
        .service(team_create)
        .service(team_join)
        .service(team_leave)
        .service(team_list)
        .service(team_mine)
        .service(crate::auth_http::logout)
}

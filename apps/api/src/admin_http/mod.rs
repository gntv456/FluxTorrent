//! M29 完整管理后台 HTTP 接口（staff 专用）。
//!
//! 覆盖（方案 M29 验收口径的 Dev 版）：种子审核队列（通过/拒绝 + 理由）、
//! 举报处理、用户管理（封禁/解封/等级调整）、审计日志查询、站点运营概览。
//! 敏感操作全部 require_staff + audit 落库（§5.7）。

pub fn mount_admin(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(admin_overview)
        .service(review_queue)
        .service(review_decide)
        .service(report_queue)
        .service(report_resolve)
        .service(report_claim)
        .service(report_release)
        .service(user_admin_list)
        .service(user_admin_detail)
        .service(admin_user_fields)
        .service(admin_user_fields_put)
        .service(user_admin_snatches)
        .service(user_grant_medal)
        .service(user_grant_item)
        .service(user_assign_jixiao)
        .service(user_admin_delete)
        .service(user_adjust)
        .service(user_flags)
        .service(user_set_status)
        .service(user_set_class)
        .service(role_list)
        .service(user_role_list)
        .service(user_role_grant)
        .service(user_role_revoke)
        .service(permission_matrix)
        .service(permission_matrix_update)
        .service(user_permission_view)
        .service(user_permission_set)
        .service(audit_query)
        .service(staff_panel)
        .service(staff_panel_entries_list)
        .service(staff_panel_entries_add)
        .service(staff_panel_entries_update)
        .service(staff_panel_entries_delete)
        .service(cheaters_scan)
        .service(site_settings_get)
        .service(site_settings_put)
        .service(agent_rules_list)
        .service(agent_rules_add)
        .service(agent_rules_del)
        .service(agent_rules_export)
        .service(agent_rules_import)
        .service(deny_reasons_list)
        .service(deny_reasons_add)
        .service(deny_reasons_update)
        .service(deny_reasons_delete)
        .service(admin_torrent_list)
        .service(torrent_op_logs)
        .service(admin_spark_logs)
        .service(admin_torrent_buys)
        .service(admin_login_logs)
        .service(forum_admin_list)
        .service(forum_admin_create)
        .service(forum_admin_update)
        .service(forum_admin_delete)
        .service(forum_mod_add)
        .service(forum_mod_remove)
        .service(forum_category_create)
        .service(forum_category_update)
        .service(forum_category_delete)
        .service(forum_category_reorder)
        // 论坛视频内嵌 V1（0189）：embed 规则 CRUD
        .service(embed_rules_list)
        .service(embed_rules_add)
        .service(embed_rules_update)
        .service(embed_rules_delete)
        // 0148 金字幕评选（候选/授金）
        .service(awards_candidates)
        .service(awards_build)
        .service(awards_grant)
        // 0149 认证字幕人（授予/撤销/列表）
        .service(certs_grant)
        .service(certs_revoke)
        .service(certs_list)
}

mod agent_rules;
mod audit;
mod deny;
mod embed_rules;
mod forum;
mod forum_check;
mod forum_mods;
mod guard;
mod overview;
mod perms;
mod records;
mod reports;
mod review;
mod roles;
mod site;
mod subtitle_awards;
mod subtitle_certs;
mod torrent_ops;
mod torrents;
mod user_del;
mod user_detail;
mod user_fields_admin;
mod user_grant;
mod user_list;
mod user_ops;
mod user_status;

pub use agent_rules::*;
pub use audit::*;
pub use deny::*;
pub use embed_rules::*;
pub use forum::*;
pub use forum_mods::*;
pub use overview::*;
pub use perms::*;
pub use records::*;
pub use reports::*;
pub use review::*;
pub use roles::*;
pub use site::*;
pub use subtitle_awards::*;
pub use subtitle_certs::*;
pub use torrent_ops::*;
pub use torrents::*;
pub use user_del::*;
pub use user_detail::*;
pub use user_fields_admin::*;
pub use user_grant::*;
pub use user_list::*;
pub use user_ops::*;
pub use user_status::*;

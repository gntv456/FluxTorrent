//! 权限键常量（与 0054 迁移的 permissions.key 一一对应，避免拼写漂移）。
//! 从 authz.rs 按域拆出；authz.rs 以 `pub mod perm;` 直接挂载，
//! 外部路径 `crate::authz::perm::X` 不变。

// 发布
pub const TORRENT_UPLOAD: &str = "torrent.upload";
pub const TORRENT_APPROVAL_AUTO: &str = "torrent.approval.auto";
/// 尚无代码生效点：付费种子改价端点未实装（价格仅发布时设置），接线时删除本 allow
#[allow(dead_code)]
pub const TORRENT_SET_PRICE: &str = "torrent.set_price";
pub const TORRENT_VIEW_ANONYMOUS: &str = "torrent.view_anonymous";
/// 尚无代码生效点（0062 迁移已标 implemented=false）：
/// 「特殊分类」概念未定义，接线时删除本 allow
#[allow(dead_code)]
pub const TORRENT_UPLOAD_SPECIAL: &str = "torrent.upload_special";
pub const TORRENT_SEE_BANNED: &str = "torrent.see_banned";
/// 转载（repost）功能整体未实现（0062 迁移已标 implemented=false），接线时删除本 allow
#[allow(dead_code)]
pub const TORRENT_REPOST: &str = "torrent.repost";
// 保种
/// 由 worker 的 hr_enforce 批量 SQL 经 user_can() 函数消费（0056/0057），
/// API 侧无常量引用点，保留以避免两边权限键拼写漂移
#[allow(dead_code)]
pub const HR_EXEMPT: &str = "hr.exempt";
pub const SEED_STATS_VIEW: &str = "seed.stats.view";
// 外联
pub const INVITES_BONUS: &str = "invites.bonus";
pub const ANNOUNCE_PUBLISH: &str = "announce.publish";
// 内容
pub const FAQ_MANAGE: &str = "faq.manage";
pub const RULES_MANAGE: &str = "rules.manage";
pub const NEWS_MANAGE: &str = "news.manage";
pub const LINKS_MANAGE: &str = "links.manage";
pub const FUN_MANAGE: &str = "fun.manage";
pub const ADS_MANAGE: &str = "ads.manage";
pub const CATEGORIES_MANAGE: &str = "categories.manage";
pub const FORUMS_MANAGE: &str = "forums.manage";
pub const POLLS_MANAGE: &str = "polls.manage";
pub const OFFERS_PROMOTE: &str = "offers.promote";
// 用户
pub const USER_WARN: &str = "user.warn";
pub const USER_STATUS: &str = "user.status";
pub const USER_FLAGS: &str = "user.flags";
pub const USER_ADJUST: &str = "user.adjust";
pub const USER_CLASS: &str = "user.class";
pub const USER_CREATE: &str = "user.create";
pub const USER_RESETPASS: &str = "user.resetpass";
pub const USER_DELETE_DISABLED: &str = "user.delete_disabled";
pub const USER_AMOUNTBONUS: &str = "user.amountbonus";
pub const USER_AMOUNTUPLOAD: &str = "user.amountupload";
pub const APPEAL_HANDLE: &str = "appeal.handle";
pub const IP_CHECK: &str = "ip.check";
pub const UPLOADERS_VIEW: &str = "uploaders.view";
pub const HR_PARDON: &str = "hr.pardon";
pub const STAFF_PANEL: &str = "staff.panel";
pub const STAFF_MESSAGE: &str = "staff.message";
// 运营
pub const STAFFMESS: &str = "staffmess";
pub const MASSMAIL: &str = "massmail";
pub const EMAILBAN_MANAGE: &str = "emailban.manage";
pub const FREELEECH_VIEW: &str = "freeleech.view";
pub const FREELEECH_MANAGE: &str = "freeleech.manage";
pub const SETTINGS_VIEW: &str = "settings.view";
pub const SETTINGS_MANAGE: &str = "settings.manage";
pub const LOCATIONS_MANAGE: &str = "locations.manage";
pub const SITEPACKS_MANAGE: &str = "sitepacks.manage";
// 系统
pub const AUDIT_VIEW: &str = "audit.view";
pub const SYSLOG_VIEW: &str = "syslog.view";
pub const DBSTATS_VIEW: &str = "dbstats.view";
pub const STATS_VIEW: &str = "stats.view";
pub const CLEANUP_RUN: &str = "cleanup.run";
pub const CLEARCACHE: &str = "clearcache";
pub const BANS_MANAGE: &str = "bans.manage";
pub const TESTIP: &str = "testip";
pub const MAXLOGIN_VIEW: &str = "maxlogin.view";
pub const PLUGINS_MANAGE: &str = "plugins.manage";
pub const AGENTS_VIEW: &str = "agents.view";
pub const NOTCONNECTABLE_VIEW: &str = "notconnectable.view";
// 第八轮 P3 管理套件（参考站后台逐页深挖落地）
pub const TORRENT_MANAGE: &str = "torrent.manage";
pub const HR_VIEW: &str = "hr.view";
pub const INVITE_VIEW: &str = "invite.view";
pub const ATTENDANCE_MANAGE: &str = "attendance.manage";
pub const MEDAL_MANAGE: &str = "medal.manage";
pub const PROP_MANAGE: &str = "prop.manage";
pub const EXAM_MANAGE: &str = "exam.manage";
pub const TASK_MANAGE: &str = "task.manage";
pub const TRACKER_MANAGE: &str = "tracker.manage";
pub const ROLES_MANAGE: &str = "roles.manage";

//! staffpanel 管理工具（hxpt faqmanage/modrules/catmanage 等复刻）。
//! 按域拆分（300 行门禁）：见各子模块头注释。

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

mod bans;
mod content;
mod custom_pages;
mod donate;
mod donate_notify;
mod emailbans;
mod ipcheck;
mod massmail;
mod pack_apply;
mod pack_core;
mod pack_save;
mod pack_snapshot;
mod pack_tags;
mod pack_types;
mod promo;
mod promo_set;
mod rules_cats;
mod site;
mod site_db;
mod sitetype;
mod stats;
mod users_ops;
mod warned;

/// setup 向导复用站型物化链路（pack_core）：跨 crate 边界的引用别名。
/// setup_http 属 http 域，不在此 pub use 展开避免路由符号污染。
pub(crate) mod setup_bridge {
    pub(crate) type PackRef = super::sitetype::SiteTypePack;
    pub(crate) use super::pack_core::apply_pack_full;
}

pub use bans::*;
pub use content::*;
pub use custom_pages::*;
pub use donate::*;
pub use donate_notify::*;
pub use emailbans::*;
pub use ipcheck::*;
pub use massmail::*;
pub use pack_apply::*;
pub use pack_save::*;
pub use promo_set::*;
pub use rules_cats::*;
pub use site::*;
pub use site_db::*;
pub use sitetype::*;
pub use stats::*;
pub use users_ops::*;
pub use warned::*;

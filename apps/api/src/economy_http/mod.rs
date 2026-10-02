//! 经济系统 HTTP 接口（M11 商店/流水/银行 + M12 签到 + M13 站免池）。
//! 按域拆分（300 行门禁）：火花账本管线（spend/earn 四入口）在 ledger.rs，
//! 商店+道具生效在 shop.rs，免费券在 vouchers.rs，银行在 bank.rs，活期与
//! 贷款在 loans.rs，签到在 checkin.rs，站免池在 pool.rs，装扮中心在
//! dressup.rs，对账+Torznab 在 recon.rs，定向众筹在 funding.rs；
//! bank_settings 助手与挂载留在此。

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

pub fn mount_economy(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(shop_items)
        .service(shop_buy)
        .service(my_spark)
        .service(my_ledger)
        .service(bank_deposit)
        .service(bank_withdraw)
        .service(bank_list)
        .service(bank_overview)
        .service(bank_status_alias)
        .service(demand_deposit)
        .service(demand_withdraw)
        .service(loan_apply)
        .service(loan_repay)
        .service(loan_history)
        .service(bank_interest_records)
        .service(checkin)
        .service(checkin_status)
        .service(pool_status)
        .service(pool_donate)
        .service(dressup_list)
        .service(dressup_wear)
        .service(my_vouchers)
        .service(voucher_use)
        .service(spark_flow_report)
        .service(torznab_caps)
        .service(torznab_search)
        .service(fundings_list)
        .service(funding_create)
        .service(funding_contribute)
        .service(funding_my)
        .service(admin_fundings_list)
        .service(admin_funding_cancel)
        // C5 经济反通胀运营面板（0226）
        .service(economy_dashboard)
}

mod bank;
mod bank_withdraw;
mod checkin;
mod dashboard;
mod dressup;
mod funding;
mod funding_contribute;
mod ledger;
mod loan_repay;
mod loans;
mod loans_apply;
mod pool;
mod recon;
mod shop;
mod shop_effects;
mod shop_list;
mod spend;
mod voucher_use;
mod vouchers;

pub use bank::*;
pub use bank_withdraw::*;
pub use checkin::*;
pub use dashboard::*;
pub use dressup::*;
pub use funding::*;
pub use funding_contribute::*;
pub use ledger::*;
pub use loan_repay::*;
pub use loans::*;
pub use loans_apply::*;
pub use pool::*;
pub use recon::*;
pub use shop::*;
// 娱乐屋奖品的「使用」侧复用商店生效链：一件物品能干什么，只在这一个地方实现。
pub(crate) use shop_effects::{apply_item_effect, has_effect};
pub use shop_list::*;
pub use spend::*;
pub use voucher_use::*;
pub use vouchers::*;

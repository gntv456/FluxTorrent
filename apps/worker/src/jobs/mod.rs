//! 定时与消费任务。
//! 按域拆分（300 行门禁）：回填 backfill.rs / announce 消费 announce.rs+announce2.rs /
//! 促销保种 promos.rs+preserve.rs / 做种 seeding.rs / announce 主链 announce_main.rs +
//! 周期结算 settle_periodic/hr/class_adj/sweep/achv_dormant / 维护 maintain.rs / 绩效 jixiao.rs /
//! 审计对账 audit.rs / 尾段结算 settle.rs / 总调度 run.rs。

mod achv_dormant;
mod announce;
mod announce_main;
pub(crate) mod audit;
mod backfill;
mod billing;
mod catalog;
mod class_adj;
mod game_coupons;
pub(crate) mod group;
mod hr;
mod hr_resolve;
mod hr_mail;
mod jixiao;
mod jixiao_loop;
mod jixiao_snap;
mod locks;
mod lottery;
pub(crate) mod mail_out;
mod maintain;
mod manual;
mod preserve;
mod process_event;
mod promo_audit;
mod promos;
mod reconcile;
mod request_expire;
mod run;
mod seeding;
mod settle;
mod settle_periodic;
mod subtitle_cert;
mod subtitle_flow;
mod sweep;

mod social_team;
mod social_team_expire;
mod usage_stats;
pub(crate) use announce::*;
pub(crate) use catalog::*;
pub(crate) use locks::*;
pub(crate) use lottery::*;
pub(crate) use manual::*;
pub(crate) use request_expire::*;

pub(crate) use reconcile::*;
pub(crate) use social_team::*;
pub(crate) use social_team_expire::*;

pub(crate) use achv_dormant::*;
pub(crate) use announce_main::*;
pub(crate) use audit::*;
pub(crate) use backfill::*;
pub(crate) use class_adj::*;
pub(crate) use game_coupons::*;
pub(crate) use hr::*;
pub(crate) use jixiao::*;
pub(crate) use maintain::*;
pub(crate) use preserve::*;
pub(crate) use promos::*;
pub(crate) use run::*;
pub(crate) use seeding::*;
pub(crate) use settle::*;
pub(crate) use settle_periodic::*;
pub(crate) use subtitle_cert::*;
pub(crate) use subtitle_flow::*;
pub(crate) use sweep::*;
pub(crate) use usage_stats::*;

#[cfg(test)]
mod tests;

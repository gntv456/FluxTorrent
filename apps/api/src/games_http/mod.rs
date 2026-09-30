//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小/九宫格 + 农场 + 每小时限次）。
//! 经济定位：四玩法一律**回收魔力**（各自 EV < 1），赔率/概率走代码常量 + 设置键双源。
//! 按域拆分（300 行门禁）：总览/历史/回合在 overview.rs，赔率与限次助手在
//! helpers.rs，刮刮乐/猜大小/九宫格在 casino.rs，农场在 farm.rs，趣味投票在 fun.rs。
//! mount_games 留在此。

mod arcade_admin;
mod arcade_admin_write;
mod arcade_backpack;
mod arcade_board;
mod arcade_cfg;
mod arcade_claim;
mod arcade_items_delete;
mod arcade_items_write;
mod arcade_meta;
mod arcade_rewards;
mod arcade_rewards_write;
mod arcade_stubs;
mod casino;
mod farm;
mod farm_actions;
mod fun;
mod helpers;
mod item_use;
mod linkage;
mod overview;
mod pool;

use arcade_admin::*;
use arcade_admin_write::*;
use arcade_claim::*;
use arcade_items_delete::*;
use arcade_items_write::*;
use arcade_meta::*;
use arcade_rewards_write::*;
use casino::*;
use farm::*;
use farm_actions::*;
use fun::*;
use item_use::*;
use linkage::*;
use overview::*;

pub fn mount_games(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(games_overview)
        .service(game_history)
        .service(game_rounds)
        .service(scratch)
        .service(guess_bigsmall)
        .service(jgg)
        .service(farm_overview)
        .service(farm_plant)
        .service(farm_water)
        .service(farm_harvest)
        .service(fun_polls)
        .service(fun_vote)
        .service(linkage_status)
        .service(use_coupon)
        .service(arcade_meta)
        .service(claim_quest)
        .service(claim_season)
        .service(arcade_overview)
        .service(arcade_pool_save)
        .service(arcade_item_save)
        .service(arcade_item_delete)
        .service(arcade_reward_save)
        .service(backpack_use)
}

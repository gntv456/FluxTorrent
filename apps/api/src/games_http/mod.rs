//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小/打地鼠 + 农场 + 每小时限次）。
//! 经济定位：玩法一律**回收魔力**（各自 EV < 1），赔率/概率走代码常量 + 设置键双源。
//! 按域拆分（300 行门禁）：总览/历史/回合在 overview.rs，赔率与限次助手在
//! helpers.rs，刮刮乐/猜大小在 casino.rs，打地鼠在 mole.rs / mole_settle.rs，
//! 农场在 farm.rs（收获彩蛋奖池在 farm_egg.rs，土地阶梯在
//! farm_land.rs / farm_land_write.rs），趣味投票在 fun.rs。
//! mount_games 留在此。

mod arcade_admin;
mod arcade_admin_write;
mod arcade_backpack;
mod arcade_board;
mod arcade_cfg;
mod arcade_claim;
mod arcade_crops;
mod arcade_games;
mod arcade_items_delete;
mod arcade_items_write;
mod arcade_leaderboard;
mod arcade_meta;
mod arcade_rewards;
mod arcade_rewards_write;
mod arcade_stubs;
mod bigsmall_props;
mod bs_room_http;
mod bs_room_store;
mod career;
mod casino;
mod casino_bigsmall;
mod collection;
mod doudizhu_http;
mod doudizhu_pvp_http;
mod farm;
mod farm_actions;
mod farm_collection;
mod farm_egg;
mod farm_land;
mod farm_land_write;
mod fishing;
mod fishing_extra;
mod fishing_pond;
mod fixed_lottery_admin_http;
mod fixed_lottery_admin_write;
mod fixed_lottery_http;
mod food_coupon;
mod fun;
// pub(crate)：gacha_http/draw 也走同一把等级门（0490）
pub(crate) mod helpers;
mod item_use;
mod linkage;
// 逐游戏等级门（0490）：helpers re-export，gacha_http 也消费
pub(crate) mod minclass;
mod mole;
mod mole_settle;
// 测试期把补结算口暴露给 games/ 下的 DB 级测试（夹具在 games::mole_db_harness，
// 而 layer_guard 对 games_http/ 新文件零容忍 SQL，测试只能落在 games/ 侧）。
#[cfg(test)]
pub(crate) use crate::games::mole_repo::settle_stale_rounds_tx;
mod game_rounds;
mod overview;
mod pet_custom;
mod pet_log;
mod pets;
mod pool;
mod pool_batch;
#[cfg(test)]
mod pool_db_tests;
mod pool_games;
mod pool_table;
mod prize_view;
mod ranch;
mod ranch_pens;

use arcade_admin::*;
use arcade_admin_write::*;
use arcade_claim::*;
use arcade_crops::*;
use arcade_items_delete::*;
use arcade_items_write::*;
use arcade_leaderboard::*;
use arcade_meta::*;
use arcade_rewards_write::*;
use bs_room_http::*;
use career::*;
use casino::*;
use casino_bigsmall::*;
use collection::*;
use doudizhu_http::*;
use doudizhu_pvp_http::*;
use farm::*;
use farm_actions::*;
use farm_collection::*;
use farm_land::*;
use farm_land_write::*;
use fishing::*;
use fishing_extra::*;
use fishing_pond::*;
use fixed_lottery_admin_http::*;
use fixed_lottery_admin_write::*;
use fixed_lottery_http::*;
use fun::*;
use game_rounds::*;
use item_use::*;
use linkage::*;
use mole::*;
use mole_settle::*;
use overview::*;
use pet_custom::*;
use pet_log::*;
use pets::*;
use pool_batch::*;
use pool_games::*;
use ranch::*;
use ranch_pens::*;

pub fn mount_games(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(games_overview)
        .service(game_history)
        .service(game_rounds)
        .service(game_career)
        .service(scratch)
        .service(guess_bigsmall)
        // 0515 倒计时房间制（期次/倒计时/本期下注）
        .service(room_state)
        .service(room_bet)
        .service(jgg)
        .service(capsule)
        .service(fruit)
        .service(wheel)
        .service(wheel_batch)
        .service(mole_start)
        .service(mole_hit)
        .service(mole_finish)
        .service(fishing_cast)
        .service(fishing_reel)
        .service(dz_state)
        .service(dz_start)
        .service(dz_action)
        .service(dz_pvp_rooms)
        .service(dz_pvp_join)
        .service(dz_pvp_leave)
        .service(dz_pvp_deal)
        .service(dz_pvp_state)
        .service(lottery_list)
        .service(lottery_detail)
        .service(lottery_join)
        .service(lottery_claim)
        .service(admin_lottery_list)
        .service(admin_lottery_create)
        .service(admin_lottery_detail)
        .service(admin_lottery_publish)
        .service(admin_lottery_cancel)
        .service(admin_lottery_draw)
        .service(admin_lottery_winners)
        .service(admin_lottery_retry)
        .service(admin_lottery_claim_review)
        .service(admin_lottery_claim_queue)
        .service(pond_state)
        .service(pond_cast)
        .service(pond_hook)
        .service(pond_settle)
        .service(pond_act)
        .service(pond_rank)
        .service(pet_status)
        .service(pet_feed)
        .service(pet_claim)
        .service(pet_customize)
        .service(pet_log)
        .service(pet_of)
        .service(fishing_rod)
        .service(fishing_rod_upgrade)
        .service(fishing_collection)
        .service(arcade_leaderboard)
        .service(farm_overview)
        .service(farm_plant)
        .service(farm_water)
        .service(farm_fertilize)
        .service(game_collection)
        .service(farm_collection)
        .service(ranch_state)
        .service(ranch_buy)
        .service(ranch_feed)
        .service(ranch_collect)
        .service(craft_start)
        .service(craft_collect)
        .service(farm_harvest)
        .service(farm_harvest_log)
        .service(farm_land_buy)
        .service(farm_land_upgrade)
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
        .service(arcade_crops_list)
        .service(arcade_crop_save)
        .service(arcade_crop_delete)
        .service(backpack_use)
}

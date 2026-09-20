//! M24 娱乐玩法 HTTP 接口（刮刮乐/猜大小/九宫格 + 农场 + 每小时限次）。
//! 经济定位：四玩法一律**回收魔力**（各自 EV < 1），赔率/概率走代码常量 + 设置键双源。
//! 按域拆分（300 行门禁）：总览/历史/回合在 overview.rs，赔率与限次助手在
//! helpers.rs，刮刮乐/猜大小/九宫格在 casino.rs，农场在 farm.rs，趣味投票在 fun.rs。
//! mount_games 留在此。

mod casino;
mod farm;
mod farm_actions;
mod fun;
mod helpers;
mod overview;

use casino::*;
use farm::*;
use farm_actions::*;
use fun::*;
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
}

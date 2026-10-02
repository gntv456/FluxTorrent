//! 娱乐玩法域：刮刮乐 + 猜大小 + 九宫格 + 农场（统一走火花交易管线）。
//!
//! **经济定位（产品决策 2026-09-19）**：娱乐玩法一律以**回收魔力**为目的，全部玩法
//! 期望回报必须 < 1（庄家优势 > 0），不允许出现 EV = 1 的中性玩法，更不允许增发：
//! - 刮刮乐 EV 0.66、九宫格 EV 0.725（各有单测锁定）；
//! - 猜大小赢面 49% / 平 2% / 输 49%，赔率必须 < 2.0 否则 EV 恒为 1
//!   （旧值 2.0：EV = 0.49×2 + 0.02 = 1.0，且「同时押大押小」可零风险对冲），
//!   现缺省 1.9（EV 0.951），见 `BIGSMALL_WIN_MULT_PERMILLE`；
//! - 农场收获期望 = 产量/种子价 × (1 + 20% 双倍)，作物表按 0.75 标定 → EV 0.90；
//!   收获之上的**彩蛋奖池**（`arcade_pools` game='farm'）只能吃剩下的 0.10，
//!   由 `validate_farm` 与写侧闸共同把关；
//! 纪律（§M24 验收）：全部经统一火花交易管线动账，限额风控内置，赔率常量化。
#![allow(dead_code)]

mod bigsmall;
mod farm;
mod farm_land;
mod jgg;
mod scratch;

#[cfg(test)]
mod farm_land_tests;
#[cfg(test)]
mod farm_tests;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod tests;

// 重导出保留原公开面（原文件 #![allow(dead_code)] 语义下的 pub fn）：
// bin crate 的 pub use 对未被外部引用的项会触发 unused_imports，显式放行。
#[allow(unused_imports)]
pub use bigsmall::{
    bigsmall_expected_value, outcome_side, roll, validate_bet,
    validate_bigsmall, DiceOutcome, Guess, BIGSMALL_WIN_MULT_PERMILLE,
    REGION_LOSE_PERMILLE, REGION_TRIPLE_PERMILLE, REGION_WIN_PERMILLE,
};
#[allow(unused_imports)]
pub use farm::{
    crop_expected_value, egg_pay, farm_total_ev, harvest_market_price,
    market_price, market_window_hours, market_window_start,
    market_window_start_with, roll_double, validate_crop, validate_farm,
    CropError, BASE_EV as FARM_BASE_EV, GROW_HOURS_MAX as FARM_GROW_MAX,
    GROW_HOURS_MIN as FARM_GROW_MIN, PLOTS as FARM_PLOTS,
};
#[allow(unused_imports)]
pub use farm_land::{
    grow_minutes, land_config, land_price, next_purchasable_slot,
    speed_permille, upgrade_price, validate_ladder, validate_purchase,
    validate_upgrade, LandConfig, LandError, DEFAULT_LAND_BASE,
    DEFAULT_LAND_RATIO, DEFAULT_MAX_PLOTS, DEFAULT_UP_BASE,
    DEFAULT_UP_RATIO, MAX_LEVEL, MAX_PLOTS_HARD_CAP, RATIO_MIN,
};
#[allow(unused_imports)]
pub use jgg::{
    draw_entry, pool_ev, validate_pool, validate_ticket_cap, Draw, EntryKind,
    PoolEntry, PoolError, FALLBACK_MULT, MULT_UNIT,
};
#[allow(unused_imports)]
pub use scratch::{scratch_pay, ScratchOutcome};

/// 单次游戏下注上限（风控：防一次性输光；0109 games_max_bet 可调，此值为缺省）
pub const MAX_BET: i64 = 1000;
/// 每用户每小时游戏次数上限（0109 games_max_plays_per_hour 可调，此值为缺省）
pub const MAX_PLAYS_PER_HOUR: i64 = 60;

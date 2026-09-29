//! 娱乐玩法域：刮刮乐 + 猜大小 + 九宫格 + 农场（统一走火花交易管线）。
//!
//! **经济定位（产品决策 2026-09-19）**：娱乐玩法一律以**回收魔力**为目的，全部玩法
//! 期望回报必须 < 1（庄家优势 > 0），不允许出现 EV = 1 的中性玩法，更不允许增发：
//! - 刮刮乐 EV 0.66、九宫格 EV 0.725（各有单测锁定）；
//! - 猜大小赢面 49% / 平 2% / 输 49%，赔率必须 < 2.0 否则 EV 恒为 1
//!   （旧值 2.0：EV = 0.49×2 + 0.02 = 1.0，且「同时押大押小」可零风险对冲），
//!   现缺省 1.9（EV 0.951），见 `BIGSMALL_WIN_MULT_PERMILLE`；
//! - 农场收获期望 = 产量/种子价 × (1 + 20% 双倍)，作物表按 0.75 标定 → EV 0.90。
//! 纪律（§M24 验收）：全部经统一火花交易管线动账，限额风控内置，赔率常量化。
#![allow(dead_code)]

mod bigsmall;
mod farm;
mod jgg;
mod scratch;

#[cfg(test)]
mod tests;

// 重导出保留原公开面（原文件 #![allow(dead_code)] 语义下的 pub fn）：
// bin crate 的 pub use 对未被外部引用的项会触发 unused_imports，显式放行。
#[allow(unused_imports)]
pub use bigsmall::{
    bigsmall_expected_value, guess_play, guess_play_with, validate_bet,
    DiceOutcome, Guess, BIGSMALL_WIN_MULT_PERMILLE,
};
#[allow(unused_imports)]
pub use farm::{
    harvest_market_price, market_price, market_window_hours,
    market_window_start, market_window_start_with, roll_double,
};
#[allow(unused_imports)]
pub use jgg::{
    draw_entry, pool_ev, validate_pool, Draw, EntryKind, PoolEntry, PoolError,
    FALLBACK_MULT,
};
#[allow(unused_imports)]
pub use scratch::{
    scratch_play, scratch_play_with, ScratchOdds, ScratchOutcome,
};

/// 单次游戏下注上限（风控：防一次性输光；0109 games_max_bet 可调，此值为缺省）
pub const MAX_BET: i64 = 1000;
/// 每用户每小时游戏次数上限（0109 games_max_plays_per_hour 可调，此值为缺省）
pub const MAX_PLAYS_PER_HOUR: i64 = 60;

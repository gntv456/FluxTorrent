//! 猜大小（1-100）：**机制**是 49/2/49 的对称三区（小 1-49、大 52-100、50/51 平局返本），
//! **参数**（每一区按什么付、付多少）在 `arcade_pools(game='bigsmall')`（迁移 0251）。
//!
//! 拆这两层的理由和九宫格/刮刮乐一样：赔率住在设置键上时，改数字既不过 EV 闸
//! 也拿不到物品位，而「猜中送一件东西」这种配置根本表达不出来。

use rand::Rng;

use super::jgg::{PoolEntry, PoolError};
use super::MAX_BET;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Guess {
    Small, // 1-49
    Big,   // 52-100
}

/// 一区应得的千分占比：机制写死，配置不许改（配错了 `validate_bigsmall` 直接拒）
pub const REGION_WIN_PERMILLE: i64 = 490;
pub const REGION_TIE_PERMILLE: i64 = 20;
pub const REGION_LOSE_PERMILLE: i64 = 490;

/// 缺省赔率（千分比）：1900 = 1.9x。现在只是播种与兜底的缺省值，
/// 运行时读的是奖池行表里 win 区那条魔力位。
///
/// **为什么不是 2x**：赢面 49%、平局 2%、输面 49%（对称区间），
/// EV = p_win × 赔率 + p_tie × 1，赔率 2.0 时 EV 恰为 1.0 —— 既不回收魔力，
/// 且「同时押大押小」可把结果完全对冲成零风险（对站无收益、对玩家无意义）。
/// 赔率 < 2.0 才能让双向对冲变成稳定负期望，也让玩法回到回收口径。
pub const BIGSMALL_WIN_MULT_PERMILLE: i64 = 1900;

/// 猜大小期望回报（回收率）：0.49 × 赔率 + 0.02 × 1（平局返本）
pub fn bigsmall_expected_value(win_mult_permille: i64) -> f64 {
    0.49 * (win_mult_permille as f64 / 1000.0) + 0.02
}

/// 掷出的点数落在哪一区（相对玩家猜的这一侧）。
/// 返回 "win" | "tie" | "lose" —— 与 `arcade_pool_entries.side` 同一套取值。
pub fn outcome_side(number: u32, guess: Guess) -> &'static str {
    if (50..=51).contains(&number) {
        return "tie";
    }
    let region = if number <= 49 {
        Guess::Small
    } else {
        Guess::Big
    };
    if region == guess {
        "win"
    } else {
        "lose"
    }
}

/// 掷点（机制：1..=100 均匀）
pub fn roll() -> u32 {
    rand::thread_rng().gen_range(1..=100)
}

/// 一张桌的三区档位必须真的铺满 49/2/49：少一个区、或多给了一点权重，
/// 都会让「猜大猜小不对称」或「平局不返本」这种机制破坏悄悄发生。
pub fn validate_bigsmall(
    win: &[PoolEntry],
    tie: &[PoolEntry],
    lose: &[PoolEntry],
) -> Result<(), PoolError> {
    for (name, rows, want) in [
        ("赢", win, REGION_WIN_PERMILLE),
        ("平", tie, REGION_TIE_PERMILLE),
        ("输", lose, REGION_LOSE_PERMILLE),
    ] {
        if rows.is_empty() {
            return Err(PoolError::RegionEmpty(name.to_string()));
        }
        let got: i64 = rows.iter().map(|e| i64::from(e.weight)).sum();
        if got != want {
            return Err(PoolError::RegionWeight(name.to_string(), got, want));
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct DiceOutcome {
    pub number: u32,
    pub player_win: bool,
    pub payout: i64,
}

pub fn validate_bet(bet: i64) -> Result<(), String> {
    if bet <= 0 {
        return Err("下注必须为正数".into());
    }
    if bet > MAX_BET {
        return Err(format!("单次下注不能超过 {MAX_BET} 魔力"));
    }
    Ok(())
}

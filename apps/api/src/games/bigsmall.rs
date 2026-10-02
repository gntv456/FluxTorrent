//! 猜大小（三骰）：**机制**是掷 3 颗六面骰（点数和 3-18），小 = 3-10、
//! 大 = 11-18（各 105/216），三颗全同为「豹子」（6/216，直接判负——押哪边
//! 都不中）。**参数**（每一区按什么付、付多少）在 `arcade_pools(game='bigsmall')`
//! （迁移 0251 建、0265 重定三骰权重）。
//!
//! 拆这两层的理由和九宫格/刮刮乐一样：赔率住在设置键上时，改数字既不过 EV 闸
//! 也拿不到物品位，而「猜中送一件东西」这种配置根本表达不出来。
//!
//! 旧机制（1-100 数域 49/2/49 三区）在 2026-10 样图对齐批改为三骰——
//! 与传统骰子玩法和样图⑥（3 颗骰 / 小 3-10 / 大 11-18 / 豹子三同）一致。

use rand::Rng;

use super::jgg::{PoolEntry, PoolError};
use super::MAX_BET;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Guess {
    Small, // 3-10（非三同）
    Big,   // 11-18（非三同）
}

/// 一区应得的千分占比：机制写死，配置不许改（配错了 `validate_bigsmall` 直接拒）。
/// 105/216 = 486.1‰（四舍五入 486）；豹子 6/216 = 27.8‰（28）。合计 1000。
pub const REGION_WIN_PERMILLE: i64 = 486;
pub const REGION_TRIPLE_PERMILLE: i64 = 28;
pub const REGION_LOSE_PERMILLE: i64 = 486;

/// 缺省赔率（千分比）：1900 = 1.9x。现在只是播种与兜底的缺省值，
/// 运行时读的是奖池行表里 win 区那条魔力位。
///
/// **为什么不是 2.07x**：赢面 105/216、豹子判负 6/216、输面 105/216，
/// EV = 105/216 × 赔率。赔率 2.07 时 EV = 1.007（>1 放水）；1.9 时
/// EV ≈ 0.924，稳定回收口，且双向对冲仍是负期望。
pub const BIGSMALL_WIN_MULT_PERMILLE: i64 = 1900;

/// 猜大小期望回报（回收率）：105/216 × 赔率（豹子不付）
pub fn bigsmall_expected_value(win_mult_permille: i64) -> f64 {
    (105.0 / 216.0) * (win_mult_permille as f64 / 1000.0)
}

/// 三骰掷点落在哪一区（相对玩家猜的这一侧）。
/// 返回 "win" | "triple" | "lose" —— 与 `arcade_pool_entries.side` 同一套取值
/// （迁移 0265 把 side 的取值域从 win/tie/lose 改成 win/triple/lose）。
pub fn outcome_side(dice: [u32; 3], guess: Guess) -> &'static str {
    if dice[0] == dice[1] && dice[1] == dice[2] {
        return "triple"; // 豹子：押哪边都判负（可被护盾道具救）
    }
    let region = if dice[0] + dice[1] + dice[2] <= 10 {
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

/// 掷 3 颗六面骰（机制：每颗 1..=6 均匀）
pub fn roll() -> [u32; 3] {
    let mut rng = rand::thread_rng();
    [
        rng.gen_range(1..=6),
        rng.gen_range(1..=6),
        rng.gen_range(1..=6),
    ]
}

/// 一张桌的三区档位必须真的铺满 486/28/486：少一个区、或多给了一点权重，
/// 都会让「猜大猜小不对称」或「豹子白送」这种机制破坏悄悄发生。
pub fn validate_bigsmall(
    win: &[PoolEntry],
    triple: &[PoolEntry],
    lose: &[PoolEntry],
) -> Result<(), PoolError> {
    for (name, rows, want) in [
        ("赢", win, REGION_WIN_PERMILLE),
        ("豹子", triple, REGION_TRIPLE_PERMILLE),
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
    /// 三颗骰面（1-6 各一颗）；前端画骰、路单显点都用它
    pub dice: [u32; 3],
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

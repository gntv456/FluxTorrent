//! 猜大小（1-100）：赢面 49% / 平 2% / 输 49%，赔率缺省 1.9x（EV 0.951）。

use rand::Rng;

use super::MAX_BET;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Guess {
    Small, // 1-49
    Big,   // 52-100
}

/// 猜中赔率（千分比，1000 = 猜中返本不赚）。缺省 1900 = 1.9x。
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

#[derive(Debug, PartialEq)]
pub struct DiceOutcome {
    pub number: u32,
    pub player_win: bool,
    pub payout: i64,
}

/// 50/51 为平局区（返本），猜中按 `win_mult_permille` 派彩
pub fn guess_play(bet: i64, guess: Guess) -> DiceOutcome {
    guess_play_with(bet, guess, BIGSMALL_WIN_MULT_PERMILLE)
}

pub fn guess_play_with(
    bet: i64,
    guess: Guess,
    win_mult_permille: i64,
) -> DiceOutcome {
    let number: u32 = rand::thread_rng().gen_range(1..=100);
    let number_region = if number <= 49 {
        Guess::Small
    } else if number >= 52 {
        Guess::Big
    } else {
        // 平局区：返本
        return DiceOutcome {
            number,
            player_win: false,
            payout: bet,
        };
    };
    let player_win = number_region == guess;
    DiceOutcome {
        number,
        player_win,
        payout: if player_win {
            bet * win_mult_permille.max(0) / 1000
        } else {
            0
        },
    }
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

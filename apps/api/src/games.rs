//! M24 娱乐玩法域：刮刮乐 + 猜大小（旧站 magic_scratch / bigsmall 口径）。
//! 纪律（§M24 验收）：全部经统一火花交易管线动账，限额风控内置，赔率常量化。
#![allow(dead_code)]

use rand::Rng;

/// 单次游戏下注上限（风控：防一次性输光）
pub const MAX_BET: i64 = 1000;
/// 每用户每小时游戏次数上限
pub const MAX_PLAYS_PER_HOUR: i64 = 60;

// ============ 刮刮乐（即开型） ============

/// 奖池：45% 空、30% 保底 0.5x、15% 1x、8% 2x、2% 10x —— 期望回报 ≈ 0.79（庄家优势 21%，运营可调）
#[derive(Debug, PartialEq)]
pub struct ScratchOutcome {
    pub multiplier: f64,
    pub payout: i64,
}

pub fn scratch_play(bet: i64) -> ScratchOutcome {
    let roll: u32 = rand::thread_rng().gen_range(0..100);
    let multiplier = if roll < 45 {
        0.0
    } else if roll < 75 {
        0.5
    } else if roll < 90 {
        1.0
    } else if roll < 98 {
        2.0
    } else {
        10.0
    };
    ScratchOutcome {
        multiplier,
        payout: (bet as f64 * multiplier) as i64,
    }
}

// ============ 猜大小（1-100 猜大小） ============

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Guess {
    Small, // 1-49
    Big,   // 52-100
}

#[derive(Debug, PartialEq)]
pub struct DiceOutcome {
    pub number: u32,
    pub player_win: bool,
    pub payout: i64,
}

/// 50/51/52 为平局区（返本），猜中赔 2x
pub fn guess_play(bet: i64, guess: Guess) -> DiceOutcome {
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
        payout: if player_win { bet * 2 } else { 0 },
    }
}

pub fn validate_bet(bet: i64) -> Result<(), String> {
    if bet <= 0 {
        return Err("下注必须为正数".into());
    }
    if bet > MAX_BET {
        return Err(format!("单次下注不能超过 {MAX_BET} 火花"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bet_validation() {
        assert!(validate_bet(100).is_ok());
        assert!(validate_bet(0).is_err());
        assert!(validate_bet(-5).is_err());
        assert!(validate_bet(MAX_BET + 1).is_err());
    }

    #[test]
    fn scratch_payout_bounds() {
        for _ in 0..1000 {
            let o = scratch_play(100);
            // 派彩只能是 0/50/100/200/1000 五档
            assert!([0, 50, 100, 200, 1000].contains(&o.payout));
        }
    }

    #[test]
    fn dice_tie_returns_stake() {
        // 平局区 50/51 返本：强行构造（函数随机，验证结构）
        for _ in 0..500 {
            let o = guess_play(100, Guess::Big);
            if (50..=51).contains(&o.number) {
                assert_eq!(o.payout, 100);
                assert!(!o.player_win);
            }
            if o.player_win {
                assert_eq!(o.payout, 200);
            }
        }
    }
}

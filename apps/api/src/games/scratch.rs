//! 刮刮乐（即开型）：档位、概率、倍数全在 `arcade_pools(game='scratch')`，
//! 这里只留「按倍率算派彩」这一件纯计算。
//!
//! 0248 之前它住在五个设置键里（`games_scratch_*_pct`），那条路上没有 EV 闸、
//! 没有跨池回查，也没有物品位可言 —— 站长改数字改出增发，只有运行时才发现。
//! 现在它和九宫格读同一张表、过同一个 `validate_pool`。

/// 一次刮开的结果。`multiplier` 是倍率（0.5 = 返本一半），`payout` 向下取整。
#[derive(Debug, PartialEq)]
pub struct ScratchOutcome {
    pub multiplier: f64,
    pub payout: i64,
}

/// 派彩 = 注额 × 千分倍率，向下取整（对站点有利的一侧）。
/// 关闸算 EV 用的是**不取整**的精确值，见 `PoolEntry::value_permille`。
pub fn scratch_pay(bet: i64, mult_permille: i64) -> ScratchOutcome {
    ScratchOutcome {
        multiplier: mult_permille as f64 / 1000.0,
        payout: bet.saturating_mul(mult_permille) / 1000,
    }
}

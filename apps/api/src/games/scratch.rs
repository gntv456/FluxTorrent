//! 刮刮乐（即开型）：奖池 45% 空、30% 保底 0.5x、15% 1x、8% 2x、2% 10x。

use rand::Rng;

/// 奖池：45% 空、30% 保底 0.5x、15% 1x、8% 2x、2% 10x —— 期望回报 0.66（庄家优势 34%，运营可调）
#[derive(Debug, PartialEq)]
pub struct ScratchOutcome {
    pub multiplier: f64,
    pub payout: i64,
}

/// 刮刮乐档位概率（百分比整数）。0109 设置键 `games_scratch_empty_pct/half_pct/one_pct`
/// 可配前三档，剩余额度按 8:2 分给 2x/10x（与缺省表一致），见 `ScratchOdds::from_parts`。
pub struct ScratchOdds {
    pub empty: u32,
    pub half: u32,
    pub one: u32,
    pub two: u32,
    pub ten: u32,
}

impl ScratchOdds {
    /// 缺省档位：45/30/15/8/2（期望回报 0.66，庄家优势 34%）
    pub const DEFAULT: ScratchOdds = ScratchOdds {
        empty: 45,
        half: 30,
        one: 15,
        two: 8,
        ten: 2,
    };

    /// 由五档可配百分比推导完整档位。规则（保证**总量恒为 100 且 EV 不变**）：
    /// - 前四档之和 ≥ 100 → 整体回落缺省（防非法配置造出必中/必增发档位）；
    /// - 10x 档填了正数且与前四档合计正好 100 → 采用站长配置；否则取**余数**（兼容旧的"余数档"行为）；
    /// - EV 复算 ≥ 1 → 整体回落缺省（P2 运行时防线）：设置键是管理员可写参数，
    ///   单测只锁死 DEFAULT 常量。余数档设计可被配置放大为增发开关——
    ///   如 (0,0,0,99,·) → two=99%/ten=1%，EV = 0.99×2 + 0.01×10 = 2.08。
    pub fn from_parts(
        empty: i64,
        half: i64,
        one: i64,
        two: i64,
        ten: i64,
    ) -> ScratchOdds {
        let (e, h, o, t) = (empty.max(0), half.max(0), one.max(0), two.max(0));
        let sum4 = e + h + o + t;
        if sum4 >= 100 {
            return ScratchOdds::DEFAULT;
        }
        let ten_eff = if ten > 0 && sum4 + ten == 100 {
            ten
        } else {
            100 - sum4
        };
        // 半点整数口径：EV(每注) = Σ(概率×倍率) = (h×0.5 + o×1 + t×2 + ten×10) / 100；
        // 放大 200 倍避免浮点：h + 2o + 4t + 20ten < 200 ⟺ EV < 1
        if h + 2 * o + 4 * t + 20 * ten_eff >= 200 {
            return ScratchOdds::DEFAULT;
        }
        ScratchOdds {
            empty: e as u32,
            half: h as u32,
            one: o as u32,
            two: t as u32,
            ten: ten_eff as u32,
        }
    }
}

pub fn scratch_play(bet: i64) -> ScratchOutcome {
    scratch_play_with(bet, &ScratchOdds::DEFAULT)
}

pub fn scratch_play_with(bet: i64, odds: &ScratchOdds) -> ScratchOutcome {
    let roll: u32 = rand::thread_rng().gen_range(0..100);
    let (e, h, o, t) = (
        odds.empty,
        odds.empty + odds.half,
        odds.empty + odds.half + odds.one,
        odds.empty + odds.half + odds.one + odds.two,
    );
    let multiplier = if roll < e {
        0.0
    } else if roll < h {
        0.5
    } else if roll < o {
        1.0
    } else if roll < t {
        2.0
    } else {
        10.0
    };
    ScratchOutcome {
        multiplier,
        payout: (bet as f64 * multiplier) as i64,
    }
}

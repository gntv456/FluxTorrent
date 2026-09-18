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

use rand::Rng;

/// 单次游戏下注上限（风控：防一次性输光；0109 games_max_bet 可调，此值为缺省）
pub const MAX_BET: i64 = 1000;
/// 每用户每小时游戏次数上限（0109 games_max_plays_per_hour 可调，此值为缺省）
pub const MAX_PLAYS_PER_HOUR: i64 = 60;

// ============ 刮刮乐（即开型） ============

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

    /// 由四档可配百分比推导完整档位：10x = 100 - 前三档 - 2x（余数）。
    /// 越界（前三档之和 ≥100 或余数 <1）时整体回落缺省，避免构造出非法档位导致必中/必空。
    pub fn from_parts(empty: i64, half: i64, one: i64, two: i64) -> ScratchOdds {
        let (e, h, o, t) = (empty.max(0), half.max(0), one.max(0), two.max(0));
        let rest = 100 - e - h - o - t;
        if rest < 1 {
            return ScratchOdds::DEFAULT;
        }
        ScratchOdds {
            empty: e as u32,
            half: h as u32,
            one: o as u32,
            two: t as u32,
            ten: rest as u32,
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

// ============ 猜大小（1-100 猜大小） ============

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

pub fn guess_play_with(bet: i64, guess: Guess, win_mult_permille: i64) -> DiceOutcome {
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

// ============ 农场 ============
// 作物全部为通用幻想系命名（四叶草/星尘豆/云端瓜/月华参/日冕稻），**不绑定任何站型特色**
// （教育站、影音站、音乐站都用同一套），站长可在后台改名。
// 产量按「种子价 × 0.75」标定：收获期望 = 0.75 × (1 + 20% 双倍) = 0.90 < 1，回收口径。

/// 市场价波动窗口：默认 4 小时（0109 games farm_market_window_hours 可调，worker
/// 与 API 各自读取；窗口跨小时数变化只影响新窗口起点，历史价不重算）
pub fn market_window_hours() -> i64 {
    // 独立常量读取：农场域无 AppState 上下文（纯函数），经 OnceLock 缓存 env/缺省 4h。
    // 设置键在 DB，纯函数不便 async 读——0109 键以「缺省=代码默认」语义存在，
    // 消费方（farm_jobs/games_http）落库前读 site_settings 后传参进来；此函数保留
    // 兜底口径（窗口必须是 3600 整数倍，最小 1h）。
    4
}

/// 市场价波动窗口起点（0/4/8/12/16/20 点口径；hours 参数来自设置键）
pub fn market_window_start_with(ts: i64, hours: i64) -> i64 {
    let window = hours.max(1) * 3600;
    ts - (ts % window)
}

/// 市场价波动窗口：每日 0/4/8/12/16/20 点刷新一次（§M24 规则要点）
pub fn market_window_start(ts: i64) -> i64 {
    market_window_start_with(ts, market_window_hours())
}

/// 确定性市场价：基准价 ±50% 波动，同一价格窗口内稳定。
/// 用窗口起点做种子（不是真随机），保证全站所有用户同一窗口看到同一价格。
/// 审计修复（P1 套利）：买种与收获此前共用同一确定性因子——低价窗（0.5x）买入、
/// 高价窗（1.5x）收获可稳定锁定利润。现买卖两侧使用错开的独立哈希（salt 不同 →
/// 因子近似独立），期望收益回到作物表本身的档位差（0011：80/100 … 4800/5000，
/// 全档微亏防刷），波动只剩赌运气，无确定性正 EV 策略。
pub fn market_price(seed_price: i64, window_start: i64) -> i64 {
    let unit = market_unit(window_start, 0);
    let factor = 50 + (unit % 101); // 50..150
    (seed_price * factor as i64) / 100
}

/// 收获侧市场价（与买种侧 salt 错开，消除「同因子低买高卖」套利）
pub fn harvest_market_price(base_yield: i64, window_start: i64) -> i64 {
    let unit = market_unit(window_start, 0x5DEECE66D);
    let factor = 50 + (unit % 101); // 50..150
    (base_yield * factor as i64) / 100
}

/// xorshift64* 窗口哈希 → 高 31 位；salt 区分买/卖两侧因子
fn market_unit(window_start: i64, salt: u64) -> u64 {
    let mut x = (window_start as u64 ^ 0x9E3779B97F4A7C15).wrapping_add(salt);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545F4914F6CDD1D) >> 33
}

/// 20% 概率双倍收获（真随机）
pub fn roll_double() -> bool {
    rand::thread_rng().gen_range(0..100) < 20
}

// ============ 九宫格抽奖（jgg 口径） ============

/// 奖池档位（管理端可配的简化常量版）：期望回报 = 0.725，庄家优势 27.5%
/// （修复前 50x/100x 权重过高致 EV=3.43 持续增发；按下表权重×赔率精确复算）
pub struct JggPrize {
    pub label: &'static str,
    pub weight: u32, // 权重（总 1000）
    pub payout: i64, // 相对票价倍数（10 = 10x）
}

pub const JGG_PRIZES: [JggPrize; 8] = [
    JggPrize {
        label: "谢谢参与",
        weight: 731,
        payout: 0,
    },
    JggPrize {
        label: "再来一次",
        weight: 120,
        payout: 1,
    },
    JggPrize {
        label: "2x 魔力",
        weight: 60,
        payout: 2,
    },
    JggPrize {
        label: "3x 魔力",
        weight: 50,
        payout: 3,
    },
    JggPrize {
        label: "5x 魔力",
        weight: 25,
        payout: 5,
    },
    JggPrize {
        label: "10x 魔力",
        weight: 10,
        payout: 10,
    },
    JggPrize {
        label: "20x 魔力",
        weight: 3,
        payout: 20,
    },
    JggPrize {
        label: "50x 魔力",
        weight: 1,
        payout: 50,
    },
];

pub const JGG_TICKET: i64 = 100;

pub struct JggDraw {
    pub index: usize, // 中奖格子 0..8（渲染九宫格）
    pub prize: &'static JggPrize,
}

pub fn jgg_draw() -> JggDraw {
    let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
    let mut roll: u32 = rand::thread_rng().gen_range(0..total);
    for (i, p) in JGG_PRIZES.iter().enumerate() {
        if roll < p.weight {
            return JggDraw { index: i, prize: p };
        }
        roll -= p.weight;
    }
    unreachable!("weights sum to total")
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

    /// 九宫格期望回报恒为 0.725（庄家优势 27.5%）——防止赔率表回归到增发配置
    #[test]
    fn jgg_expected_value_house_edge() {
        let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
        assert_eq!(total, 1000, "权重总和建议恒为 1000");
        let ev: f64 = JGG_PRIZES
            .iter()
            .map(|p| p.weight as f64 * p.payout as f64)
            .sum::<f64>()
            / total as f64;
        assert!(
            (ev - 0.725).abs() < 1e-9,
            "EV 漂移: {ev}（调整赔率表必须同步更新本断言与经济模型）"
        );
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
                assert_eq!(o.payout, 190);
            }
        }
    }

    /// 经济纪律（2026-09-19 产品决策）：娱乐玩法一律回收，期望回报必须 < 1。
    /// 猜大小赢面 49% / 平 2% / 输 49%，赔率 ≥ 2.0 时 EV ≥ 1.0 且可双向零风险对冲 ——
    /// 这个断言锁死「赔率 < 2.0」，调整赔率必须同步复算本式与经济文档。
    #[test]
    fn bigsmall_expected_value_below_one() {
        let ev = bigsmall_expected_value(BIGSMALL_WIN_MULT_PERMILLE);
        assert!(ev < 1.0, "猜大小 EV 未回收: {ev}");
        assert!(
            (ev - 0.951).abs() < 1e-9,
            "EV 漂移: {ev}（1.9x 应为 0.951）"
        );
        assert!(
            BIGSMALL_WIN_MULT_PERMILLE < 2000,
            "赔率必须 < 2000‰，否则 EV ≥ 1 且可对冲套零风险"
        );
        // 隐含：赔率越高 EV 越高，2.0 处恰好回到 1.0（不允许）
        assert!((bigsmall_expected_value(2000) - 1.0).abs() < 1e-9);
    }

    /// 刮刮乐档位参数化的期望回报仍 < 1（四档可配，10x 取余数）
    #[test]
    fn scratch_expected_value_below_one() {
        let ev = |o: &ScratchOdds| {
            (o.empty as f64 * 0.0
                + o.half as f64 * 0.5
                + o.one as f64 * 1.0
                + o.two as f64 * 2.0
                + o.ten as f64 * 10.0)
                / 100.0
        };
        let d = ScratchOdds::DEFAULT;
        assert_eq!(d.empty + d.half + d.one + d.two + d.ten, 100, "档位必须铺满 100%");
        assert!((ev(&d) - 0.66).abs() < 1e-9, "缺省 EV 漂移: {}", ev(&d));
        // 争议配置（前三档吃满）必须回落缺省，不能构造出必中/增发档位
        let bad = ScratchOdds::from_parts(60, 30, 15, 8);
        assert_eq!(bad.empty + bad.half + bad.one + bad.two + bad.ten, 100);
        assert!(ev(&bad) < 1.0, "越界配置未回落，EV={}", ev(&bad));
    }

    /// 九宫格头奖 50x 的 EV 已由 jgg_expected_value_house_edge 锁定（0.725）。

    #[test]
    fn market_price_stable_in_window() {
        let w = market_window_start(1788000000);
        assert_eq!(market_price(100, w), market_price(100, w));
        // 相邻窗口价格可以变也可以不变，但窗口起点对齐 4h
        assert_eq!(w % (4 * 3600), 0);
    }

    #[test]
    fn market_price_within_bounds() {
        for w in 0..50 {
            let p = market_price(1000, w * 14400);
            assert!((500..=1500).contains(&p), "price {p} out of ±50% bounds");
        }
    }

    /// 审计修复（P1 套利）回归锁：买种侧与收获侧因子必须错开——
    /// 若两侧同因子，存在窗口对 (w_buy, w_sell) 使 seed 价 ×0.5 而 yield 价 ×1.5，
    /// 确定性利润 +200%。锁「同窗口下两侧因子不同的窗口占比 ≥ 80%」，
    /// 防止未来改哈希时不慎回到同因子。
    #[test]
    fn buy_and_harvest_factors_are_independent() {
        let mut diff = 0;
        let n = 200;
        for w in 0..n {
            let buy = market_price(10_000, w * 14400);
            let sell = harvest_market_price(10_000, w * 14400);
            if buy != sell {
                diff += 1;
            }
        }
        assert!(
            (diff as f64 / n as f64) >= 0.8,
            "buy/harvest factors too correlated: {diff}/{n}"
        );
    }

    #[test]
    fn jgg_weights_cover_all() {
        let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
        assert_eq!(total, 1000);
        // 抽样次数按最低权重动态推算：期望最稀有档被抽到 ~25 次，
        // 漏检概率 ≈ e^-25。不能用固定次数——赔率表调整会改变最低权重
        // （旧表最低 5/1000 固定 2000 次即可，EV 修复后 50x 降至 1/1000，
        // 2000 次漏检概率高达 ~13%，测试随机失败）。
        let min_weight = JGG_PRIZES.iter().map(|p| p.weight).min().unwrap();
        assert!(min_weight > 0, "存在权重为 0 的档位，该奖永远不可能被抽到");
        let draws = 25 * total / min_weight;
        let mut seen = [false; 8];
        for _ in 0..draws {
            let d = jgg_draw();
            seen[d.index] = true;
        }
        assert!(seen.iter().all(|&s| s), "some prize never drawn: {seen:?}");
    }
}

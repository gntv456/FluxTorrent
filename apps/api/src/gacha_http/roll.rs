//! 抽取 roll 工具（G31-B）：与 `gacha_math::simulate` 同式的加权选择，
//! 保证实测出金率收敛到公示综合概率（方案 §5 断言「10 万抽落在抽样误差内」）。
//! 纯函数 + 可注入 rnd，便于单测；UP 卡选择（guarantee_up/up_ratio）留
//! G31-C 与包载荷一起落（种子池无 up_card_id）。

use crate::gacha_math;

/// 单抽 roll：软保底爬坡（q(s)=gs+boost，从非金档按比例搬）+ 硬保底强制，
/// 加权累计选择；返回奖池行下标。`since` = 本抽开始前未出金连续抽数。
pub(super) fn roll(
    cfg: &gacha_math::PoolConfig,
    since: i32,
    rnd: &mut impl FnMut() -> f64,
) -> usize {
    let hard = cfg.pity.hard.max(1.0) as i32;
    let soft = cfg.pity.soft;
    let ramp = cfg.pity.ramp;
    let gold: Vec<bool> = cfg
        .rates
        .iter()
        .map(|x| gacha_math::is_gold(&x.r))
        .collect();
    let w_sum: f64 = cfg.rates.iter().map(|x| x.w).sum();
    let w_sum = if w_sum != 0.0 { w_sum } else { 1.0 };
    let gs: f64 = cfg
        .rates
        .iter()
        .enumerate()
        .filter(|(i, _)| gold[*i])
        .map(|(_, x)| x.w / w_sum)
        .sum();
    let non_gs = 1.0 - gs;
    if since >= hard {
        // 硬保底：只在金档内按权重选
        let r = rnd() * gs;
        let mut acc = 0.0;
        let mut pick = cfg.rates.len() - 1;
        for (k, x) in cfg.rates.iter().enumerate() {
            if !gold[k] {
                continue;
            }
            acc += x.w / w_sum;
            if r < acc {
                pick = k;
                break;
            }
        }
        return pick;
    }
    let b = if since as f64 > soft {
        ((since as f64 - soft) * ramp / 100.0).min(if gs != 0.0 {
            1.0 - gs
        } else {
            0.0
        })
    } else {
        0.0
    };
    let probs: Vec<f64> = cfg
        .rates
        .iter()
        .enumerate()
        .map(|(k, x)| {
            let pi = x.w / w_sum;
            if gold[k] {
                pi * (1.0 + if gs != 0.0 { b / gs } else { 0.0 })
            } else {
                pi * (1.0 - if non_gs != 0.0 { b / non_gs } else { 0.0 })
            }
        })
        .collect();
    let tot: f64 = probs.iter().fold(0.0, |a, x| a + x.max(0.0));
    let r = rnd() * tot;
    let mut acc = 0.0;
    let mut pick = probs.len() - 1;
    for (k, p) in probs.iter().enumerate() {
        acc += p.max(0.0);
        if r < acc {
            pick = k;
            break;
        }
    }
    pick
}

/// 批种子：纳秒 ^ user_id 混乘（行种子 = 批种子 + seq，落库可复核）。
pub(super) fn batch_seed(user_id: i64) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    t ^ (user_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// pity_at 落库口径：金档记 0（重置），非金档记本抽后的连续抽数。
pub(super) fn pity_at(since_after: i32, gold: bool) -> i32 {
    if gold {
        0
    } else {
        since_after
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gacha_math::{PityConfig, PoolConfig, RateRow};

    fn demo_cfg() -> PoolConfig {
        PoolConfig {
            rates: vec![
                RateRow {
                    r: "MISS".into(),
                    kind: "miss".into(),
                    w: 10.0,
                    value: 0.0,
                    shards: 0.0,
                    dupe: 0.0,
                    synth: 0.0,
                    lv_max: 1.0,
                    lv_step: 0.0,
                    lv_gain: 0.0,
                },
                RateRow {
                    r: "SHARD".into(),
                    kind: "shard".into(),
                    w: 12.0,
                    value: 0.0,
                    shards: 15.0,
                    dupe: 0.0,
                    synth: 0.0,
                    lv_max: 1.0,
                    lv_step: 0.0,
                    lv_gain: 0.0,
                },
                RateRow {
                    r: "R".into(),
                    kind: "card".into(),
                    w: 50.0,
                    value: 8.0,
                    shards: 0.0,
                    dupe: 8.0,
                    synth: 60.0,
                    lv_max: 5.0,
                    lv_step: 12.0,
                    lv_gain: 0.2,
                },
                RateRow {
                    r: "SSR".into(),
                    kind: "card".into(),
                    w: 2.0,
                    value: 200.0,
                    shards: 0.0,
                    dupe: 60.0,
                    synth: 520.0,
                    lv_max: 10.0,
                    lv_step: 40.0,
                    lv_gain: 0.5,
                },
            ],
            pity: PityConfig {
                soft: 50.0,
                hard: 65.0,
                ramp: 5.0,
                up_ratio: None,
                guarantee_up: false,
            },
        }
    }

    /// 与公示综合概率互验：20 万抽蒙特卡洛，出金率须落在抽样误差内
    /// （同方案 §5「10 万抽」断言的小型crate内版）。
    #[test]
    fn mc_converges_to_composite() {
        let cfg = demo_cfg();
        let dp = gacha_math::composite(&cfg);
        let mut rnd = gacha_math::rng(42);
        let n = 200_000i32;
        let mut since = 0i32;
        let mut gold_n = 0i64;
        for _ in 1..=n {
            since += 1;
            let pick = roll(&cfg, since, &mut rnd);
            if gacha_math::is_gold(&cfg.rates[pick].r) {
                gold_n += 1;
                since = 0;
            }
        }
        let mc = gold_n as f64 / n as f64;
        assert!(
            (mc - dp.gold_comp).abs() < 0.005,
            "MC {mc} vs DP {}",
            dp.gold_comp
        );
    }

    /// 硬保底必中金档：since=hard 时 roll 必返 gold 行。
    #[test]
    fn hard_pity_forces_gold() {
        let cfg = demo_cfg();
        let mut rnd = gacha_math::rng(7);
        for _ in 0..50 {
            let pick = roll(&cfg, 65, &mut rnd);
            assert!(gacha_math::is_gold(&cfg.rates[pick].r));
        }
    }

    /// pity_at 口径：金档归零、非金档保留。
    #[test]
    fn pity_at_semantics() {
        assert_eq!(pity_at(0, true), 0);
        assert_eq!(pity_at(17, false), 17);
    }
}

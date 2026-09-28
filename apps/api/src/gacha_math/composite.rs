//! 保底 DP 精确解：q(s) = s≥hard ? 1 : gs + boost(s)；
//! h(s) = 期望经过 s 的次数；comp(i) = Σ_s h(s)·P(i|s) / Σ_s h(s)，
//! 质量守恒 Σ comp == 1。状态 s = 周期内已抽数（1-based）。

use super::{is_gold, PoolConfig};

pub struct CompositeResult {
    /// 表定概率（归一化权重）。
    pub base: Vec<f64>,
    pub gold_base: f64,
    /// 含保底的综合概率 —— 公示、守卫、EV 全用这份。
    pub comp: Vec<f64>,
    pub gold_comp: f64,
    /// 一个保底周期的期望长度（抽数）。
    pub cycle: f64,
    /// 走满硬保底的概率（纯装饰时的后台告警口径）。
    pub hard_prob: f64,
    pub up_rate: f64,
    pub up_share: f64,
    /// Σ comp·value。
    pub ev_comp: f64,
    /// 金档占比为 0 或 ≥1 时保底 DP 无意义，comp 退化为 base。
    pub degenerate: bool,
}

pub fn composite(cfg: &PoolConfig) -> CompositeResult {
    let rates = &cfg.rates;
    let w_sum: f64 = rates.iter().map(|x| x.w).sum();
    let w = if w_sum != 0.0 { w_sum } else { 1.0 };
    let p: Vec<f64> = rates.iter().map(|x| x.w / w).collect();
    let gs = p.iter().enumerate().fold(0.0, |a, (i, x)| {
        a + if is_gold(&rates[i].r) { *x } else { 0.0 }
    });
    let soft = cfg.pity.soft;
    let hard = f64::max(1.0, cfg.pity.hard);
    let ramp = cfg.pity.ramp;
    let up_ratio = match cfg.pity.up_ratio {
        None => 1.0,
        Some(v) => v.max(0.0).min(1.0),
    };
    let mut out = CompositeResult {
        base: p.clone(),
        gold_base: gs,
        comp: p.clone(),
        gold_comp: gs,
        cycle: 1.0,
        hard_prob: 0.0,
        up_rate: 0.0,
        up_share: 0.0,
        ev_comp: 0.0,
        degenerate: true,
    };
    if gs <= 0.0 || gs >= 1.0 || !gs.is_finite() {
        out.ev_comp = p
            .iter()
            .enumerate()
            .fold(0.0, |a, (i, pi)| a + pi * rates[i].value);
        return out;
    }
    out.degenerate = false;

    let boost = |s: f64| -> f64 {
        if s > soft && s < hard {
            ((s - soft) * ramp / 100.0).min(1.0 - gs)
        } else {
            0.0
        }
    };
    let q = |s: f64| -> f64 {
        if s >= hard {
            1.0
        } else {
            (gs + boost(s)).min(1.0)
        }
    };
    let mut h: Vec<f64> = Vec::new();
    let mut cur = 1.0;
    let mut s = 1.0;
    while s <= hard {
        h.push(cur);
        cur *= 1.0 - q(s);
        s += 1.0;
    }
    let big_l = h.iter().sum::<f64>();
    let big_l = if big_l != 0.0 { big_l } else { 1.0 };
    let non_gold_share = 1.0 - gs;
    out.comp = p
        .iter()
        .enumerate()
        .map(|(i, pi)| {
            let gold = is_gold(&rates[i].r);
            h.iter().enumerate().fold(0.0, |acc, (s, hs)| {
                let st = s as f64 + 1.0;
                let b = boost(st);
                if gold {
                    acc + hs
                        * (if st >= hard {
                            pi / gs
                        } else {
                            pi * (1.0 + b / gs)
                        })
                } else {
                    acc + hs
                        * (if st >= hard {
                            0.0
                        } else {
                            pi * (1.0
                                - if non_gold_share != 0.0 {
                                    b / non_gold_share
                                } else {
                                    0.0
                                })
                        })
                }
            }) / big_l
        })
        .collect();
    let mut hard_prob = 1.0;
    let mut s = 1.0;
    while s < hard {
        hard_prob *= 1.0 - q(s);
        s += 1.0;
    }
    // UP 命中率：硬保底给的金按 guarantee_up 必中，其余按 up_ratio。
    let gold_pulls = h
        .iter()
        .enumerate()
        .fold(0.0, |acc, (s, hs)| acc + hs * q(s as f64 + 1.0))
        / big_l;
    let up_pulls = h.iter().enumerate().fold(0.0, |acc, (s, hs)| {
        let st = s as f64 + 1.0;
        let r = if st >= hard && cfg.pity.guarantee_up {
            1.0
        } else {
            up_ratio
        };
        acc + hs * q(st) * r
    }) / big_l;
    out.gold_comp = out.comp.iter().enumerate().fold(0.0, |a, (i, x)| {
        a + if is_gold(&rates[i].r) { *x } else { 0.0 }
    });
    out.cycle = big_l;
    out.hard_prob = hard_prob;
    out.up_share = if gold_pulls != 0.0 {
        up_pulls / gold_pulls
    } else {
        0.0
    };
    out.up_rate = up_pulls;
    out.ev_comp = out
        .comp
        .iter()
        .enumerate()
        .fold(0.0, |a, (i, x)| a + x * rates[i].value);
    out
}

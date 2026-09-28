//! 蒙特卡洛模拟：与 `composite` 互为对照，两侧实现必须给出同一整数计数
//! （mulberry32 纯整数流，跨语言逐位同谱）。

use super::{is_gold, rng, PoolConfig};

pub struct SimulateResult {
    pub freq: Vec<f64>,
    /// 逐档命中计数（整数，跨语言逐位可对账）。
    pub counts: Vec<i64>,
    /// 出金间隔（自上一金起的抽数）与最大间隔。
    pub gaps: Vec<i64>,
    pub max_gap: i64,
    pub gold: i64,
    pub hard_hits: i64,
    pub ten_win: i64,
    pub ten_tot: i64,
    pub ten_rate: f64,
    pub ev_sim: f64,
    pub ev_base: f64,
    pub gold_base: f64,
    pub n: i64,
}

pub fn simulate(cfg: &PoolConfig, n: i64, seed: i64) -> SimulateResult {
    let rates = &cfg.rates;
    let base: Vec<f64> = rates.iter().map(|x| x.w).collect();
    let w_sum = base.iter().sum::<f64>();
    let w = if w_sum != 0.0 { w_sum } else { 1.0 };
    let p: Vec<f64> = base.iter().map(|x| x / w).collect();
    let gold: Vec<bool> = rates.iter().map(|x| is_gold(&x.r)).collect();
    let gs = p
        .iter()
        .enumerate()
        .fold(0.0, |a, (i, x)| a + if gold[i] { *x } else { 0.0 });
    let soft = cfg.pity.soft;
    let hard = f64::max(1.0, cfg.pity.hard);
    let ramp = cfg.pity.ramp;
    let mut rnd = rng(seed);
    let mut counts = vec![0i64; rates.len()];
    let mut gaps: Vec<i64> = Vec::new();
    let mut since = 0.0f64;
    let mut gold_n = 0i64;
    let mut hard_hits = 0i64;
    let mut ten_win = 0i64;
    let mut ten_tot = 0i64;
    let mut max_gap = 0i64;
    let mut batch: Vec<usize> = Vec::new();
    let non_gs = 1.0 - gs;
    for _ in 1..=n {
        since += 1.0;
        let mut pick: usize;
        if since >= hard {
            let r = rnd() * gs;
            let mut acc = 0.0;
            pick = rates.len() - 1;
            for (k, pk) in p.iter().enumerate() {
                if !gold[k] {
                    continue;
                }
                acc += pk;
                if r < acc {
                    pick = k;
                    break;
                }
            }
            hard_hits += 1;
        } else {
            let b = if since > soft {
                ((since - soft) * ramp / 100.0).min(if gs != 0.0 {
                    1.0 - gs
                } else {
                    0.0
                })
            } else {
                0.0
            };
            let probs: Vec<f64> = p
                .iter()
                .enumerate()
                .map(|(k, pi)| {
                    if gold[k] {
                        pi * (1.0 + if gs != 0.0 { b / gs } else { 0.0 })
                    } else {
                        pi * (1.0
                            - if non_gs != 0.0 { b / non_gs } else { 0.0 })
                    }
                })
                .collect();
            let tot = probs.iter().fold(0.0, |a, x| a + x.max(0.0));
            let r = rnd() * tot;
            let mut acc = 0.0;
            pick = probs.len() - 1;
            for (k, pk) in probs.iter().enumerate() {
                acc += pk.max(0.0);
                if r < acc {
                    pick = k;
                    break;
                }
            }
        }
        counts[pick] += 1;
        if gold[pick] {
            let g = since as i64;
            if g > max_gap {
                max_gap = g;
            }
            gaps.push(g);
            gold_n += 1;
            since = 0.0;
        }
        batch.push(pick);
        if batch.len() == 10 {
            if batch.iter().any(|k| gold[*k]) {
                ten_win += 1;
            }
            ten_tot += 1;
            batch.clear();
        }
    }
    let freq: Vec<f64> = counts.iter().map(|c| *c as f64 / n as f64).collect();
    let ev_sim = rates
        .iter()
        .enumerate()
        .fold(0.0, |a, (i, x)| a + freq[i] * x.value);
    let ev_base = p
        .iter()
        .enumerate()
        .fold(0.0, |a, (i, pi)| a + pi * rates[i].value);
    SimulateResult {
        freq,
        counts,
        gaps,
        max_gap,
        gold: gold_n,
        hard_hits,
        ten_win,
        ten_tot,
        ten_rate: if ten_tot != 0 {
            ten_win as f64 / ten_tot as f64
        } else {
            0.0
        },
        ev_sim,
        ev_base,
        gold_base: gs,
        n,
    }
}

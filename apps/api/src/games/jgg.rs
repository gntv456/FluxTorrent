//! 九宫格抽奖（jgg 口径）：期望回报必须 < 1（回收口纪律）。
//!
//! **奖池是数据，不是代码**：档位来自 `arcade_pools` / `arcade_pool_entries`
//! （迁移 0242 播种）。本模块只做纯计算与校验，不引 sqlx —— DB 访问按现有分层
//! 留在 `games_http/`。EV 公式在这里只有一份，admin / overview / 测试都用它，
//! 不再各自抄一遍求和。

use rand::Rng;

/// 单个奖池档位。`payout` 是**相对票价的倍数**（0 = 不中），
/// 与 `arcade_pool_entries.payout` 同口径，所以 EV = Σ(权重×倍数)/Σ权重 与票价无关。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JggPrize {
    pub label: String,
    pub weight: u32,
    pub payout: i64,
}

pub struct JggDraw {
    pub index: usize, // 中奖格子 0..n-1（渲染九宫格）
    pub prize: JggPrize,
}

/// 奖池校验失败的原因。文案直接给运营看，所以要具体到档位。
#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    Empty,
    ZeroWeight(String),
    WeightsOverflow(u64),
    /// EV ≥ 1：这个玩法不再回收魔力，而是在增发 —— 一律关闸，不回落
    ExpectedValueNotBelowOne(f64),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::Empty => write!(f, "奖池为空：没有任何启用档位"),
            PoolError::ZeroWeight(l) => write!(f, "档位「{l}」权重为 0，永远抽不到"),
            PoolError::WeightsOverflow(t) => write!(f, "权重合计 {t} 超出 u32 抽样范围"),
            PoolError::ExpectedValueNotBelowOne(ev) => {
                write!(f, "综合返还 {ev:.3} ≥ 1：玩法在增发而非回收，拒绝服务")
            }
        }
    }
}

/// 综合返还（EV）。空池返回 0 —— 由 `validate_pool` 负责拒绝空池，这里不 panic。
pub fn jgg_ev(prizes: &[JggPrize]) -> f64 {
    let total: u64 = prizes.iter().map(|p| u64::from(p.weight)).sum();
    if total == 0 {
        return 0.0;
    }
    let sum: f64 = prizes
        .iter()
        .map(|p| f64::from(p.weight) * p.payout as f64)
        .sum();
    sum / total as f64
}

/// 关闸式校验：不合法的池子一律拒绝，**不猜旧值、不回落缺省**。
/// 静默回落等于把「运营改错一个字」伪装成「配置生效了」。
pub fn validate_pool(prizes: &[JggPrize]) -> Result<(), PoolError> {
    if prizes.is_empty() {
        return Err(PoolError::Empty);
    }
    let mut total: u64 = 0;
    for p in prizes {
        if p.weight == 0 {
            return Err(PoolError::ZeroWeight(p.label.clone()));
        }
        total += u64::from(p.weight);
    }
    if total > u64::from(u32::MAX) {
        return Err(PoolError::WeightsOverflow(total));
    }
    let ev = jgg_ev(prizes);
    if !(ev < 1.0) {
        return Err(PoolError::ExpectedValueNotBelowOne(ev));
    }
    Ok(())
}

/// 按权重抽一档。调用方必须先过 `validate_pool`；这里只多一道防御性 None，
/// 绝不退化成「随便给一档」。
pub fn jgg_draw_from(prizes: &[JggPrize]) -> Option<JggDraw> {
    let total: u64 = prizes.iter().map(|p| u64::from(p.weight)).sum();
    if total == 0 || total > u64::from(u32::MAX) {
        return None;
    }
    let mut roll: u64 = rand::thread_rng().gen_range(0..total);
    for (i, p) in prizes.iter().enumerate() {
        if roll < u64::from(p.weight) {
            return Some(JggDraw {
                index: i,
                prize: p.clone(),
            });
        }
        roll -= u64::from(p.weight);
    }
    None
}

//! 娱乐玩法奖池：纯计算 + 关闸校验，不引 sqlx。
//!
//! **奖池是数据不是代码**：档位来自 `arcade_pools` / `arcade_pool_entries`（迁移 0242），
//! 物品位引用 `arcade_items`（迁移 0244）。DB 访问按现有分层留在 `games_http/`。
//!
//! EV 口径：一档位的**价值**是它给玩家的魔力等值 —— 魔力位是 `票价 × 倍数`，
//! 物品位是 `arcade_items.anchor × 件数`。EV = Σ(权重 × 价值) / Σ权重 / 票价。
//! 物品价值只从 anchor 派生，绝不取「登记价」：登记价可以被改小，anchor 不行。

use rand::Rng;

/// 档位种类。物品位的价值不在自己身上，在目录的 anchor 上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// 魔力位：`multiples` 是票价倍数（0 = 不中）
    Magic { multiples: i64 },
    /// 物品位：`anchor` 是加载时从 `arcade_items` 解析出的单件魔力等值
    Item {
        item_key: String,
        qty: i32,
        anchor: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolEntry {
    pub label: String,
    pub weight: u32,
    pub kind: EntryKind,
}

/// 物品库存耗尽 / 达每人上限时的回落：返票价倍数魔力。EV 也按这一侧兜底 ——
/// 物品即使发不出去，站点该付的钱已经在账上。
pub const FALLBACK_MULT: i64 = 0;

impl PoolEntry {
    /// 该档位的魔力等值。取「物品价值」与「回落价值」的较大者，是对玩家更有利的
    /// 那一侧：用有利侧算 EV 仍然 < 1，才是真的小于 1。
    pub fn value(&self, ticket: i64) -> i64 {
        match &self.kind {
            EntryKind::Magic { multiples } => ticket.saturating_mul(*multiples),
            EntryKind::Item { qty, anchor, .. } => {
                let v = anchor.saturating_mul(i64::from(*qty));
                v.max(ticket.saturating_mul(FALLBACK_MULT))
            }
        }
    }
}

pub struct Draw {
    pub index: usize,
    pub prize: PoolEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    Empty,
    ZeroWeight(String),
    WeightsOverflow(u64),
    /// 物品位引用了目录里不存在 / 已停用 / 无折算价的物品 —— 发不出去的空头承诺
    UnknownItem(String),
    BadTicket(i64),
    /// EV ≥ 1：不再回收魔力而是在增发 —— 一律关闸，不回落
    ExpectedValueNotBelowOne(f64),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::Empty => write!(f, "奖池为空：没有任何启用档位"),
            PoolError::ZeroWeight(l) => {
                write!(f, "档位「{l}」权重为 0，永远抽不到")
            }
            PoolError::WeightsOverflow(t) => {
                write!(f, "权重合计 {t} 超出 u32 抽样范围")
            }
            PoolError::UnknownItem(k) => {
                write!(
                    f,
                    "物品位引用目录中不存在、已停用或无折算价的物品「{k}」"
                )
            }
            PoolError::BadTicket(t) => write!(f, "票价必须为正，实为 {t}"),
            PoolError::ExpectedValueNotBelowOne(ev) => {
                write!(f, "综合返还 {ev:.3} ≥ 1：玩法在增发而非回收，拒绝服务")
            }
        }
    }
}

/// 综合返还率（EV）。空池返回 0 —— 由 `validate_pool` 拒绝空池，这里不 panic。
pub fn pool_ev(entries: &[PoolEntry], ticket: i64) -> f64 {
    let total: u64 = entries.iter().map(|e| u64::from(e.weight)).sum();
    if total == 0 || ticket <= 0 {
        return 0.0;
    }
    let sum: f64 = entries
        .iter()
        .map(|e| f64::from(e.weight) * e.value(ticket) as f64)
        .sum();
    sum / total as f64 / ticket as f64
}

/// 关闸式校验：不合法的池子一律拒绝，**不猜旧值、不回落缺省**。
/// 静默回落等于把「运营改错一个字」伪装成「配置生效了」。
pub fn validate_pool(
    entries: &[PoolEntry],
    ticket: i64,
) -> Result<(), PoolError> {
    if entries.is_empty() {
        return Err(PoolError::Empty);
    }
    if ticket <= 0 {
        return Err(PoolError::BadTicket(ticket));
    }
    let mut total: u64 = 0;
    for e in entries {
        if e.weight == 0 {
            return Err(PoolError::ZeroWeight(e.label.clone()));
        }
        if let EntryKind::Item {
            item_key, anchor, ..
        } = &e.kind
        {
            if item_key.trim().is_empty() || *anchor <= 0 {
                return Err(PoolError::UnknownItem(item_key.clone()));
            }
        }
        total += u64::from(e.weight);
    }
    if total > u64::from(u32::MAX) {
        return Err(PoolError::WeightsOverflow(total));
    }
    let ev = pool_ev(entries, ticket);
    if !(ev < 1.0) {
        return Err(PoolError::ExpectedValueNotBelowOne(ev));
    }
    Ok(())
}

/// 按权重抽一档。调用方必须先过 `validate_pool`；这里只多一道防御性 None，
/// 绝不退化成「随便给一档」。
pub fn draw_entry(entries: &[PoolEntry]) -> Option<Draw> {
    let total: u64 = entries.iter().map(|e| u64::from(e.weight)).sum();
    if total == 0 || total > u64::from(u32::MAX) {
        return None;
    }
    let mut roll: u64 = rand::thread_rng().gen_range(0..total);
    for (i, e) in entries.iter().enumerate() {
        if roll < u64::from(e.weight) {
            return Some(Draw {
                index: i,
                prize: e.clone(),
            });
        }
        roll -= u64::from(e.weight);
    }
    None
}

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
    /// 魔力位：`mult_permille` 是票价倍数 ×1000（0 = 不中；500 = 0.5 倍）
    Magic { mult_permille: i64 },
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

/// 一千 = 1.0 倍：把 0.5x 这类分数倍数带进整数列（刮刮乐有返本一半档）
pub const MULT_UNIT: i64 = 1000;

impl PoolEntry {
    /// **精确**价值 ×1000。EV 一律按它算：向下取整只对站点有利，
    /// 用对玩家更有利的一侧算出来仍然 < 1，才是真的小于 1。
    /// 取「物品价值」与「回落价值」的较大者，同一条保守口径。
    pub fn value_permille(&self, ticket: i64) -> i64 {
        match &self.kind {
            EntryKind::Magic { mult_permille } => {
                ticket.saturating_mul(*mult_permille)
            }
            EntryKind::Item { qty, anchor, .. } => {
                let v = anchor
                    .saturating_mul(i64::from(*qty))
                    .saturating_mul(MULT_UNIT);
                let fb = ticket
                    .saturating_mul(FALLBACK_MULT)
                    .saturating_mul(MULT_UNIT);
                v.max(fb)
            }
        }
    }

    /// 该档位的魔力等值（实际派彩口径，向下取整）
    pub fn value(&self, ticket: i64) -> i64 {
        self.value_permille(ticket) / MULT_UNIT
    }

    /// 千分倍率（物品位恒为 0：它的价值不在自己身上，在目录的 anchor 上）
    pub fn mult_permille(&self) -> i64 {
        match &self.kind {
            EntryKind::Magic { mult_permille } => *mult_permille,
            EntryKind::Item { .. } => 0,
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
    /// 猜大小少了一个区（赢/平/输）—— 机制被配置破坏
    RegionEmpty(String),
    /// 某一区的权重不等于机制规定的千分占比
    RegionWeight(String, i64, i64),
    /// EV ≥ 1：不再回收魔力而是在增发 —— 一律关闸，不回落
    ExpectedValueNotBelowOne(f64),
    /// 票档 / 最低注额高过「单次下注上限」：没有任何合法注额能开出这一局
    TicketAboveMaxBet {
        ticket: i64,
        max_bet: i64,
    },
    /// 票档非正（农场那池是定标单位，另三个是最低注额）
    BadTicketValue(i64),
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
            PoolError::RegionEmpty(n) => write!(
                f, "猜大小缺「{n}」区的档位：三区（赢/平/输）都必须配上才是一张桌"
            ),
            PoolError::RegionWeight(n, got, want) => write!(
                f, "猜大小「{n}」区权重合计 {got}，机制规定 {want}（49/2/49 的千分占比）"
            ),
            PoolError::ExpectedValueNotBelowOne(ev) => {
                write!(f, "综合返还 {ev:.3} ≥ 1：玩法在增发而非回收，拒绝服务")
            }
            PoolError::TicketAboveMaxBet { ticket, max_bet } => write!(
                f,
                "票价 / 最低注额 {ticket} 超过单次下注上限 {max_bet}：没有任何合法注额，玩法当场死掉，拒绝保存"
            ),
            PoolError::BadTicketValue(t) => {
                write!(f, "票价 / 定标单位必须为正，实为 {t}")
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
        .map(|e| f64::from(e.weight) * e.value_permille(ticket) as f64)
        .sum();
    sum / total as f64 / (ticket as f64 * MULT_UNIT as f64)
}

/// 精确比较「池子 EV < cap_num / cap_den」。
///
/// 边界必须用整数判：浮点会把「0.90 + 0.10」算成 0.9999999999999999，
/// 于是一张**恰好中性**的池子从 `!(total < 1.0)` 底下溜过去 —— 那正是
/// 「EV=1 的中性池」最该被拒的形状。判据写作
/// `Σ(权重 × 等值千分) × cap_den < cap_num × Σ权重 × 票价 × 1000`，
/// 两侧都是整数，不引入容差，也不改变对站点有利的那一侧取整口径。
pub fn ev_strictly_below(
    entries: &[PoolEntry],
    ticket: i64,
    cap_num: i64,
    cap_den: i64,
) -> bool {
    let mut total_w: i128 = 0;
    let mut sum: i128 = 0;
    for e in entries {
        let w = i128::from(e.weight);
        total_w += w;
        sum += w * i128::from(e.value_permille(ticket));
    }
    if total_w == 0 || ticket <= 0 || cap_den <= 0 || cap_num < 0 {
        return false;
    }
    sum * i128::from(cap_den)
        < i128::from(cap_num) * total_w * i128::from(ticket) * 1000
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
    validate_shape(entries, ticket)?;
    // 判据用整数比较（见 `ev_strictly_below`），浮点值只用来报账
    if !ev_strictly_below(entries, ticket, 1, 1) {
        return Err(PoolError::ExpectedValueNotBelowOne(pool_ev(
            entries, ticket,
        )));
    }
    Ok(())
}

/// 票档上限关闸：`games_max_bet` 是风控常数，票价 / 最低注额超过它，
/// 「低于票档拒开」与「高于上限拒开」两条规则就把所有注额都堵死了 ——
/// 玩法不是变贵，是不再可玩。运行时 `casino.rs` 同样会拒开这一局，
/// 但那等于把配置事故留给玩家发现，所以写侧先拦。
pub fn validate_ticket_cap(ticket: i64, max_bet: i64) -> Result<(), PoolError> {
    if ticket <= 0 {
        return Err(PoolError::BadTicketValue(ticket));
    }
    if max_bet > 0 && ticket > max_bet {
        return Err(PoolError::TicketAboveMaxBet { ticket, max_bet });
    }
    Ok(())
}

/// 池子的**结构**项：票价、权重、物品位有效性。EV 判据各玩法不同（农场在
/// 奖池之外还有 0.90 的确定性收获），所以不在这里 —— 但结构项只此一份：
/// 每个玩法抄一遍的话，「物品位 anchor 必须为正」这类检查迟早漏一处。
pub fn validate_shape(
    entries: &[PoolEntry],
    ticket: i64,
) -> Result<(), PoolError> {
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

//! 商城三段式履约的状态机（PT Studio FR-SHOP 对标，FRAME §7 第 6 条）。
//!
//! **冻结 → 履约 → 扣款**：钱在履约成功前一直是冻结态。
//!
//! 【为什么不直接「扣款 → 履约」】那样一旦在两者之间崩溃，钱已经扣了
//! 而东西没发出去，只能等用户自己重试才可能补发；不重试就是永久的
//! 「花钱买空气」。冻结态让中断可安全回滚，而且冻结对用户可见——
//! 「待确认订单」列表天然就有了。
//!
//! 【与旧口径的关系】不动 `effect_applied`（效果是否到位）与
//! `spark_ledger`（已扣款）的既有语义，只在其前面插一层钱的阶段。
//! 两条判据互补：新状态管「钱到哪一步」，老列管「效果发没发」。

use crate::errors::{DomainError, DomainResult};

/// 履约状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 占位中（刚建单，还没冻结）
    Reserving,
    /// 已冻结、待履约（钱被占住但没扣）
    Frozen,
    /// 已履约并扣款（终态）
    Done,
    /// 履约失败，冻结待解冻
    Failed,
    /// 冻结已解冻退款（终态）
    Refunded,
}

impl State {
    /// 解析数据库值。未知值一律按 `Done` 处理——
    /// 绝不把「读不懂的状态」当成未完成去重试：那会重复发货。
    pub fn parse(s: &str) -> Self {
        match s {
            "reserving" => Self::Reserving,
            "frozen" => Self::Frozen,
            "failed" => Self::Failed,
            "refunded" => Self::Refunded,
            _ => Self::Done,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Reserving => "reserving",
            Self::Frozen => "frozen",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Refunded => "refunded",
        }
    }

    /// 是否是终态（不再需要恢复任务推进）。
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Refunded)
    }
}

/// 推进一步应做什么。
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// 落冻结（还没冻结时）。
    Freeze { amount: i64 },
    /// 执行履约并扣款。
    Fulfill,
    /// 解冻退款（履约失败 / 宽限超时且重试仍失败）。
    Release { amount: i64, reason: String },
    /// 什么都不做（已终态，或还没到推进时机）。
    None,
}

/// 一次履约的输入。
#[derive(Debug, Clone)]
pub struct Input {
    pub state: State,
    /// 应冻结/已冻结的金额。
    pub amount: i64,
    /// 履约是否成功（由 `apply_item_effect` 的结果决定）。
    pub fulfill_ok: bool,
    /// 已冻结时长（秒）——用于宽限判定。
    pub frozen_secs: i64,
    /// 站点宽限（秒）。
    pub grace_secs: i64,
    /// 本轮是不是「恢复任务的重试」而非用户主动购买。
    pub is_retry: bool,
}

/// 状态机推进一格。
///
/// 关键时序：**先解冻再判定**，保证任何非 `Done` 的终态都不会留着钱。
/// 这条不变量是整个设计的承重点——有了它，「钱去哪了」永远可答。
pub fn advance(i: &Input) -> Action {
    // 终态：永远不动。重复发货比状态不漂亮严重得多。
    if i.state.is_terminal() {
        return Action::None;
    }
    match i.state {
        State::Reserving => Action::Freeze { amount: i.amount },
        State::Frozen => {
            if i.fulfill_ok {
                return Action::Fulfill;
            }
            // 宽限未过 → 不急着退款，等下一轮（可能 apply 慢，也可能
            // 用户刷新页面正常触发同一笔）。宽限已过 → 解冻，钱不能
            // 永久卡在冻结态。
            if i.frozen_secs < i.grace_secs && !i.is_retry {
                Action::None
            } else {
                Action::Release {
                    amount: i.amount,
                    reason: "fulfill_failed".into(),
                }
            }
        }
        // Failed → 恢复任务接手：重试履约一次，再失败就退款。
        // 单次重试是刻意的：同一个必然失败的履约（配置错、库存没了）
        // 无限重试只是刷日志。
        State::Failed => {
            if i.fulfill_ok {
                Action::Fulfill
            } else {
                Action::Release {
                    amount: i.amount,
                    reason: "fulfill_failed_after_retry".into(),
                }
            }
        }
        State::Done | State::Refunded => Action::None,
    }
}

/// 计算订单总价（含递增价格）。
///
/// 递增规则：已售 `sold` 份、阶梯起点 `anchor`、每份涨 `step`、
/// 封顶 `max`。`max` 为 None 时必须给一个硬上限保护——否则配置漏填
/// 会让单价随销量无限膨胀，一算就是天文数字。
///
/// `max` 为 `None` 时**直接报错**而不是偷偷用兜底上限——配置漏填属于
/// 站长失误，报错让他看见；偷偷封顶只会得到「明明配了递增却卖不动」
/// 这种查不出原因的现场。
pub fn price_with_step(
    unit: i64,
    sold: i64,
    step: Option<i64>,
    max: Option<i64>,
    anchor: i32,
) -> DomainResult<i64> {
    // 负数校验必须在 step 提前返回**之前**：否则「单价 -1、step 未配」
    // 这条最危险的配置反而绕过了检查（负单价会反向给用户加钱）。
    if unit < 0 || sold < 0 {
        return Err(DomainError::Validation(
            "单价与已售量不能为负".into(),
        ));
    }
    let Some(step) = step else {
        return Ok(unit);
    };
    // 超过 anchor 的部分每份涨 step
    let over = (sold - anchor as i64).max(0);
    let raw = unit
        .checked_add(step.checked_mul(over).ok_or_else(|| {
            DomainError::Validation("递增价格超出可计算范围".into())
        })?)
        .ok_or_else(|| {
            DomainError::Validation("递增价格超出可计算范围".into())
        })?;
    let cap = max.ok_or_else(|| {
        DomainError::Validation(
            "启用递增价格必须设上限 price_max（否则单价会无限膨胀）".into(),
        )
    })?;
    Ok(raw.min(cap).max(0))
}

/// 校验递增价格配置（配错会让限量商品要么永不涨价、要么天价）。
pub fn validate_step(
    step: Option<i64>,
    max: Option<i64>,
    anchor: i32,
) -> DomainResult<()> {
    if let Some(s) = step {
        if s < 0 {
            return Err(DomainError::Validation(
                "递增步长不能为负（负数等于降价）".into(),
            ));
        }
        if s > 0 && max.is_none() {
            return Err(DomainError::Validation(
                "启用递增价格必须设上限 price_max——否则单价会无限膨胀"
                    .into(),
            ));
        }
    }
    if let Some(m) = max {
        if m < 0 {
            return Err(DomainError::Validation(
                "价格上限不能为负".into(),
            ));
        }
    }
    if anchor < 0 {
        return Err(DomainError::Validation(
            "阶梯起点不能为负".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

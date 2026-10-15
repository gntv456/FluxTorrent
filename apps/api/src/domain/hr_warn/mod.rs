//! H&R 警告顺延与处置阶梯的判定（FRAME §7 第 6 条）。
//!
//! PT Studio FR-HR 对标 + 评估报告 §2.1：
//! **警告 → 限制下载 → 自动恢复 → 可选封禁**，且警告期禁下新种 /
//! 禁发邀请。
//!
//! 【最关键的一条：Gazelle 式顺延】
//! 新警告从**现有警告的到期时刻**往后顺延，而不是从今天重算。
//! 为什么：若从今天重算，连续违规的人每次都是「14 天警告」而永远踩在
//! 同一天到期，警告期形同虚设；顺延则违规次数越多、解冻越晚，与
//! 「警告要真的有威慑力」的意图一致。
//!
//! 【顺延不是无限累加】
//! 设上限（`max_shift_days`）：否则一个反复违规的人能被推到几年后，
//! 而站点其实希望他「改过就回来」。超过上限时**封顶到上限并提示人工**——
//! 自动推到无期，等于把处置权从公式手里交给了一个没人审计的累加器。

/// 警告阶梯的一档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarnStep {
    /// 该档的警告天数（基准）。
    pub days: i32,
    /// 该档是否限制下载。
    pub restrict_download: bool,
    /// 该档是否禁下新种。
    pub block_upload: bool,
    /// 该档是否禁发邀请。
    pub block_invite: bool,
}

/// 一条警告的处置输入。
#[derive(Debug, Clone, Copy)]
pub struct WarnInput {
    /// 该用户**未解决**的警告条数（本次之前）。
    pub open_warnings: i32,
    /// 最早一条未解决警告的到期时刻（没有则为 None）。
    pub earliest_expiry: Option<chrono::DateTime<chrono::Utc>>,
    /// 本档基准天数。
    pub base_days: i32,
    /// 顺延上限（天）。0 = 不封顶。
    pub max_shift_days: i32,
    /// 现在。
    pub now: chrono::DateTime<chrono::Utc>,
}

/// 一次警告的处置结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarnDecision {
    /// 本次警告的到期时刻。
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// 实际顺延了多少天（0 = 从今天起算）。
    pub shifted_days: i32,
    /// 是否撞到顺延上限（撞到就提示人工介入）。
    pub hit_cap: bool,
    /// 命中的阶梯下标（按未解决警告数选档）。
    pub step: usize,
}

/// 阶梯（越往后越严）。
pub const LADDER: &[WarnStep] = &[
    WarnStep {
        days: 7,
        restrict_download: false,
        block_upload: false,
        block_invite: false,
    },
    WarnStep {
        days: 14,
        restrict_download: true,
        block_upload: false,
        block_invite: false,
    },
    WarnStep {
        days: 30,
        restrict_download: true,
        block_upload: true,
        block_invite: false,
    },
    WarnStep {
        days: 90,
        restrict_download: true,
        block_upload: true,
        block_invite: true,
    },
];

/// 按未解决警告条数取阶梯档（超出则停在最严档）。
pub fn step_for(open_warnings: i32) -> (usize, WarnStep) {
    // 第一次警告（open=0）落在第 0 档。
    let idx = open_warnings.max(0) as usize;
    let idx = idx.min(LADDER.len() - 1);
    (idx, LADDER[idx])
}

/// 判定一次警告的到期时刻（Gazelle 式顺延）。
pub fn decide(i: &WarnInput) -> WarnDecision {
    let (step, s) = step_for(i.open_warnings);
    // 基准天数取「阶梯天数」与「本档 base_days」的较大者——
    // 站长配的 base_days 应当能覆盖阶梯，不会被阶梯压低。
    let days = s.days.max(i.base_days).max(1);

    // 顺延起点：现有最早一条未解决警告的到期时刻（没有则从现在）。
    let base = i.earliest_expiry.unwrap_or(i.now);
    let mut shifted_days = 0i32;
    let mut expires = base + chrono::Duration::days(days as i64);

    // 顺延量的口径：「比一条**从现在起算**的裸警告晚多少天」。
    //
    // 三个量分开算，别混：
    //   fresh  = now + days            裸警告（没有在先警告时的到期）
    //   expires = base + days           顺延后（在先警告到期之后再加 days）
    //   shifted = expires - fresh       顺延了多少天
    //
    // 两处必须钳：
    //   1. **下限** `expires >= fresh`。在先警告可能早已到期（base < now），
    //      此时 `base + days` 会早于裸警告——等于「多违规一次反而判得更轻」。
    //      必须抬回 fresh。
    //   2. **上限** `shifted <= max_shift_days`（配了才生效）。顺延不封顶
    //      会把反复违规的人推到几年后；而自动推到无期，等于把处置权交给
    //      一个没人审计的累加器。故封顶并报 hit_cap，交人工。
    let fresh = i.now + chrono::Duration::days(days as i64);
    if let Some(base_t) = i.earliest_expiry {
        let natural = base_t + chrono::Duration::days(days as i64);
        expires = natural.max(fresh);
        shifted_days =
            (expires - fresh).num_days() as i32;
        if i.max_shift_days > 0 && shifted_days > i.max_shift_days {
            shifted_days = i.max_shift_days;
            expires = fresh + chrono::Duration::days(shifted_days as i64);
        }
    }
    WarnDecision {
        expires_at: expires,
        shifted_days,
        hit_cap: i.max_shift_days > 0 && shifted_days >= i.max_shift_days,
        step,
    }
}

/// 是否应自动恢复（全部警告已到期且解除）。
///
/// 注意：只有「所有」未解决警告都过了到期时刻才恢复。判据是
/// `max(到期)`，不是 `min`——用 min 会让最早一条到期就放人，
/// 而他还有三条违规挂着。
pub fn should_auto_restore(
    open_expiries: &[chrono::DateTime<chrono::Utc>],
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    if open_expiries.is_empty() {
        return true;
    }
    // 一条都没到期：open_expiries 非空且 max 仍大于 now → 不恢复。
    open_expiries.iter().all(|e| *e <= now)
}

/// 该用户当前是否处于「限制下载」态。
pub fn is_download_restricted(
    open_expiries: &[chrono::DateTime<chrono::Utc>],
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    // 任一未到期警告所在档位要求限制下载即受限。
    // 这里用「有没有未到期的」，保守从严：宁可多限一会儿。
    open_expiries.iter().any(|e| *e > now)
}

/// 配置校验失败（双 crate include! 复用，故自带错误类型）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidWarn(pub String);

impl std::fmt::Display for InvalidWarn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for InvalidWarn {}

/// 校验顺延上限配置。
pub fn validate_max_shift(max_shift_days: i32) -> Result<(), InvalidWarn> {
    if max_shift_days < 0 {
        return Err(InvalidWarn(
            "顺延上限不能为负（负数等于禁罚）".into(),
        ));
    }
    if max_shift_days > 3650 {
        return Err(InvalidWarn(
            "顺延上限不得超过 3650 天（10 年）——再长就等于无期".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

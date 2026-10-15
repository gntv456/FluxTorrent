//! 勋章自动成就判定（PT Studio FR-MEDAL-001~022 对标，FRAME §7 第 6 条）。
//!
//! 双模式：
//!   * **event**：事件计数——做种/发布/评论… 累加到阈值（做种按小时计，
//!     一条事件可带 amount，故「一次做种 10 小时」能按量累加）；
//!   * **threshold**：统计阈值——上传量/做种总时长/注册天数达线，周期性
//!     整体重算（不做增量）。
//!
//! 【为什么不做「全部走 event」】统计类指标（累计上传量）没有逐次事件，
//! 硬造事件就得全量回放历史流量表——成本高且容易与真实值漂移。
//! 两条路分开，各用各的取数口径（event 取事件流，threshold 取现值快照）。
//!
//! 【回填的语义边界】回填只补「现在已经达标的人」，不追溯发奖。
//! 追溯会让「本周才完成目标的人昨天就领到了徽章」这种时间悖论；
//! 且回填要读全站用户 × 全站成就，必须可中断可续跑（见 backfill_cursor）。

/// 判定模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 非成就（手动授予的普通勋章）。
    Manual,
    /// 事件计数。
    Event,
    /// 统计阈值。
    Threshold,
}

impl Mode {
    /// 解析数据库里的 `auto_mode`。未知值回落 `Manual`——
    /// 配错时宁可不自动发，也不要用错误的规则发错奖。
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("event") => Self::Event,
            Some("threshold") => Self::Threshold,
            _ => Self::Manual,
        }
    }
}

/// 配置校验失败。
///
/// 本模块被 api 与 worker 双 crate 以 `#[path]` include! 复用，故不能
/// 引用某一侧的 `DomainError`——改为自带一个最小错误类型，由调用方包成
/// 各自域的错误（同 exam_fair 的处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidConfig(pub String);

impl std::fmt::Display for InvalidConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for InvalidConfig {}

/// 统计周期（threshold 模式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Total,
    Day,
    Week,
    Month,
}

impl Period {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("day") => Self::Day,
            Some("week") => Self::Week,
            Some("month") => Self::Month,
            // total 与未知值都回落累计：周期配置错不该让阈值口径变松
            _ => Self::Total,
        }
    }

    /// 距离当前周期起点的天数（Total 恒 0）。
    pub fn window_days(&self, now: chrono::DateTime<chrono::Utc>) -> i64 {
        use chrono::Datelike;
        match self {
            Self::Total => 0,
            Self::Day => 0,
            Self::Week => now.weekday().num_days_from_monday() as i64,
            Self::Month => now.day() as i64 - 1,
        }
    }
}

/// 一条成就规则。
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub mode: Mode,
    pub period: Period,
    /// 达成阈值（event = 次数；threshold = 统计值）。
    pub target: i64,
}

/// 判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 未达标，且无需写库（进度没变）。
    Pending,
    /// 未达标但进度变了 —— 需落 `medal_progress.current`。
    Progressed { current: i64 },
    /// 达标 —— 授予（幂等由 `idx_user_medals_auto` 兜底）。
    Award,
    /// 已授予过，无需动作（避免重复发）。
    AlreadyAwarded,
}

/// 事件模式判定：把本次事件累加到进度上。
///
/// `current` 允许**超过** target 后继续累加：进度条要显示真实值
/// （「超额 250 小时」比「刚好 100」有信息量），达标只看 `>=`。
pub fn judge_event(rule: &Rule, current: i64, amount: i64) -> Verdict {
    if rule.mode != Mode::Event {
        return Verdict::Pending;
    }
    let next = current.saturating_add(amount.max(0));
    if next < rule.target {
        return Verdict::Progressed { current: next };
    }
    Verdict::Award
}

/// threshold 模式判定：整体重算，无增量。
pub fn judge_threshold(
    rule: &Rule,
    current: i64,
    awarded: bool,
) -> Verdict {
    if rule.mode != Mode::Threshold {
        return Verdict::Pending;
    }
    if awarded {
        return Verdict::AlreadyAwarded;
    }
    if current >= rule.target {
        return Verdict::Award;
    }
    Verdict::Progressed { current }
}

/// 阈值模式的进度比例（0.0 ~ 1.0，封顶 1.0）。
///
/// 超额不该显示成 130%——进度条溢出后前端会画出界。
pub fn progress_ratio(current: i64, target: i64) -> f64 {
    if target <= 0 {
        // target 配 0 或负数时视为「零门槛即达成」，比例给满，
        // 但判定本身仍以 `>=` 为准（0 >= 0 为真，会 Award）。
        return 1.0;
    }
    let r = current as f64 / target as f64;
    r.clamp(0.0, 1.0)
}

/// 展示用的进度文案：`当前 / 目标（单位）`。
///
/// 抽成函数是为了让「前端画进度」与「后台配文案」用同一套格式化——
/// 之前每处自己拼，出现过「100 GB / 50 GB」这种读起来矛盾的口径。
pub fn progress_text(current: i64, target: i64, unit: &str) -> String {
    if unit.is_empty() {
        format!("{current} / {target}")
    } else {
        format!("{current} / {target} {unit}")
    }
}

/// 校验成就配置（后台配错会让成就永远达不成或永远达成）。
pub fn validate_rule(
    mode: Mode,
    target: i64,
    period: Option<Period>,
) -> Result<(), InvalidConfig> {
    if mode == Mode::Manual {
        return Ok(());
    }
    if target <= 0 {
        return Err(InvalidConfig(
            "自动成就的阈值必须为正数（否则人人立刻达成或永不可达）".into(),
        ));
    }
    if mode == Mode::Threshold && period.is_none() {
        return Err(InvalidConfig(
            "统计阈值模式必须指定统计周期".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

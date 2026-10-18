//! H&R 分档要求 + 做种减免的判定（FRAME §7 第 6 条）。
//!
//! PT Studio FR-HR 对标 + 评估报告 §2.1：大站的护肝核心是「你多做了种，
//! 门槛就低一点」，而不是裸比分享率把人逼到删种。
//!
//! 【两条不可让步的边界】
//! 1. **减免只降门槛、不降责任**：减免后 `required_hours` 仍须 >= 下限
//!    （默认 1 小时）。减到 0 等于「做种多就不用 H&R」，会被刷种者直接
//!    利用——这是这套机制最容易被攻破的一点，故钉在单测里。
//! 2. **阶梯取最靠后一档、不叠加**：多条 relief 命中时取 extra_hours 最大
//!    的那条。叠加会让「做种多」变成指数级收益，与设计意图相反。
//!
//! ⚠️ 运行期**不走这里**：建快照是 `worker/src/jobs/hr_snapshot.rs` 的一条
//! INSERT…SELECT，按行调 Rust 不现实，所以它调 SQL 侧镜像函数
//! `hr_reduced_seed_hours` / `hr_reduced_days`（0385 落、0394 修）。本模块只剩
//! 单测在用 —— 于是「两侧同式」这件事没有任何运行时约束：P-335 就是 SQL 侧
//! 少除一次 100，把成员义务放大 100 倍，而两侧单测各自全绿。改这里的公式必须
//! 同步改 SQL 函数，并跑 `scripts/pt_audit_cz_hr_tier_math.py`（矩阵逐格比对两侧）。

/// 分档定义（倍率而非绝对值：种子的 hr_policy 仍是基线）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    /// 做种小时倍率（百分比）。100 = 不变；60 = 只要求 60%。
    pub seed_hours_pct: i32,
    /// 考察天数倍率。
    pub days_pct: i32,
}

/// 做种减免阶梯的一档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relief {
    /// 超过该额外做种小时数即享此档。
    pub extra_hours: i32,
    /// 减免百分比（分档倍率再乘 (100 - relief_pct)）。
    pub relief_pct: i32,
}

/// 减免后的 H&R 要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirement {
    /// 实际要求的做种小时。
    pub seed_hours: i32,
    /// 实际考察天数。
    pub days: i32,
    /// 命中了哪一档减免（None = 无减免，便于前台解释「为什么降了」）。
    pub relief_tier: Option<i32>,
}

/// 按等级取分档。
///
/// `tiers` 是 `档名 -> Tier` 的表，`map` 是 `class_id -> 档名` 的映射。
///
/// **未映射的等级返回 `None`** —— 也就是「不减免，按种子原值」：
/// 配置缺失绝不能被当成放宽。那是这条函数最容易写反的地方：
/// 一个 `unwrap_or(&default_relaxed)` 就能把 LV1 的严苛档悄悄变成豁免。
pub fn tier_of<'a>(
    class_id: i32,
    map: &[(i32, String)],
    tiers: &'a [(&str, Tier)],
) -> Option<&'a Tier> {
    let name = map
        .iter()
        .find(|(cid, _)| *cid == class_id)
        .map(|(_, n)| n.as_str())?;
    tiers.iter().find(|(n, _)| *n == name).map(|(_, t)| t)
}

/// 计算减免后的 H&R 要求。
///
/// 口径顺序（顺序本身就是设计）：
///   1. 种子 hr_policy 基线（seed_hours / days）；
///   2. 等级分档倍率（严格档 150% / 减免档 60% / 默认 100%）；
///   3. 做种减免（额外做种小时 → 阶梯减免，取最靠后一档不叠加）；
///   4. 地板钳制（默认 1 小时）——**只降门槛不降责任**。
pub fn requirement(
    base_seed_hours: i32,
    base_days: i32,
    tier: Option<Tier>,
    relief_ladder: &[Relief],
    // 用户本档周期内累计做种小时
    user_seed_hours: i32,
    // 减免后门槛下限（小时）
    floor_hours: i32,
) -> Requirement {
    // 1+2：分档倍率
    let (pct, days_pct) = match tier {
        Some(t) => (t.seed_hours_pct, t.days_pct),
        None => (100, 100),
    };
    let scaled = base_seed_hours.saturating_mul(pct) / 100;
    // 先乘后除再钳下限——写成 `.max(1) / 100` 会算出 0 天义务
    // （1 × 10% → 10/100 = 0），那等于「买了但不用还」。
    let days = (base_days.saturating_mul(days_pct) / 100).max(1);

    // 3：额外做种小时 → 减免（取 extra_hours 最大的一档，不叠加）
    let extra = user_seed_hours.saturating_sub(base_seed_hours).max(0);
    let hit = relief_ladder
        .iter()
        .filter(|r| extra >= r.extra_hours)
        .max_by_key(|r| r.extra_hours)
        .copied();
    let relief_pct = hit.map(|r| r.relief_pct).unwrap_or(0);
    let relieved = scaled.saturating_mul(100 - relief_pct) / 100;

    // 4：地板钳制——减免只降门槛不降责任
    let seed_hours = relieved.max(floor_hours.max(0));
    Requirement {
        seed_hours,
        days,
        relief_tier: hit.map(|r| r.extra_hours),
    }
}

/// 校验分档配置（配错会让人被莫名严苛要求或莫名豁免）。
pub fn validate_tier(pct: i32, days_pct: i32) -> Result<(), InvalidConfig> {
    if !(0..=400).contains(&pct) {
        return Err(InvalidConfig("分档做种倍率须在 0-400% 之间".into()));
    }
    if !(10..=400).contains(&days_pct) {
        return Err(InvalidConfig("分档天数倍率须在 10-400% 之间".into()));
    }
    Ok(())
}

/// 配置校验失败（本模块被 api 与 worker 双 crate include! 复用，
/// 故不能引用某一侧的 DomainError——同 medal / exam_fair 的处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidConfig(pub String);

impl std::fmt::Display for InvalidConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for InvalidConfig {}

#[cfg(test)]
mod tests;

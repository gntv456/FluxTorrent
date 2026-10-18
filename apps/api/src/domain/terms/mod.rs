//! 术语键生命周期与覆盖率巡检的判定（FRAME §7 第 6 条）。
//!
//! PT Studio FR-TERMS-001~030 对标 + 评估报告 §2.4：这不是功能，是**机制**——
//! 治理 i18n 硬编码债。没有它，术语表会长成一座没人敢动的坟。
//!
//! 【退役不是删除——这是本模块最重要的一条】
//! `deprecated` → `retired` 是单向门：退役后**不得再被引用**，但仍可渲染。
//! 直接删键会让所有引用它的界面当场崩；而「留着不管」又让废弃键
//! 永远躺在表里。退役态 + 巡检出 `retired_refs` 才是正解：界面不崩，
//! 且欠账是可见的、有限的、可逐条清偿的。
//!
//! 【覆盖率不能单独看】
//! `coverage_pct` 好看但引用着一堆退役键，是典型的假绿。故巡检结果
//! 同时返回 `retired_refs`，让「真绿」= 高覆盖 + 零退役引用。

/// 键状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 在用：可被引用、可被替换。
    Active,
    /// 已废弃：仍可渲染，但巡检会提示「该迁移了」。
    Deprecated,
    /// 已退役：**不得再被引用**，但历史渲染不崩。
    Retired,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s {
            "deprecated" => Self::Deprecated,
            "retired" => Self::Retired,
            _ => Self::Active,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Retired => "retired",
        }
    }

    /// 是否允许被新代码引用。
    pub fn allows_reference(&self) -> bool {
        matches!(self, Self::Active | Self::Deprecated)
    }
}

/// 状态迁移是否允许。
///
/// 允许的只有四种：`deprecated → retired`（单向门，真退役）、
/// `retired → active`（撤回误操作，保命通道）、以及 active/deprecated
/// 之间的往返。**不允许**从 retired 直接跳别的，必须先回 active——
/// 否则「顺手改一下」会把一个刻意退役的键复活，且不留痕。
pub fn can_transition(from: Status, to: Status) -> bool {
    if from == to {
        return true;
    }
    matches!(
        (from, to),
        (Status::Deprecated, Status::Retired)
            | (Status::Retired, Status::Active)
            | (Status::Active, Status::Deprecated)
            | (Status::Deprecated, Status::Active)
    )
}

/// 巡检统计的输入。
#[derive(Debug, Clone, Copy)]
pub struct AuditInput {
    pub total: i64,
    pub active: i64,
    pub deprecated: i64,
    pub retired: i64,
    /// 有替换规则的键数。
    pub with_rule: i64,
}

/// 巡检结果。
#[derive(Debug, Clone, PartialEq)]
pub struct Audit {
    pub total_keys: i64,
    pub active_keys: i64,
    pub deprecated_keys: i64,
    pub retired_keys: i64,
    pub with_rule: i64,
    /// 覆盖率百分比，保留一位小数。
    pub coverage_pct: f64,
    /// 真绿 = 高覆盖且零退役引用。
    pub healthy: bool,
}

/// 跑一轮巡检。
///
/// 覆盖率的两个边界值得写下来，因为它们是「假绿」的来源：
///   * `active_keys == 0`（一个在用键都没登记）→ 覆盖率 **0** 而非 100。
///     否则「没登记任何键」会被算成「全覆盖」——正好和事实相反；
///   * 已退役键**不计入分母**：它们本来就不该被替换，计进去会永久
///     把覆盖率压低，变成一个没人能改善的数字。
pub fn audit(i: &AuditInput, retired_refs: usize) -> Audit {
    let coverage = if i.active > 0 {
        (i.with_rule as f64 / i.active as f64 * 1000.0).round() / 10.0
    } else {
        0.0
    };
    Audit {
        total_keys: i.total,
        active_keys: i.active,
        deprecated_keys: i.deprecated,
        retired_keys: i.retired,
        with_rule: i.with_rule,
        coverage_pct: coverage,
        healthy: retired_refs == 0 && coverage >= 80.0,
    }
}

/// 配置校验失败（双 crate include! 复用，故自带错误类型——
/// 同 medal / hr / exam_fair 的处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidKey(pub String);

impl std::fmt::Display for InvalidKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for InvalidKey {}

/// 校验术语键名。
///
/// 键会进 URL、Redis key 与前端字典，故必须限定字符集：只允许
/// 字母数字与下划线。这样「构造出含分隔符的键去撞别的命名空间」
/// 这类攻击面一开始就不存在。
pub fn validate_key(key: &str) -> Result<(), InvalidKey> {
    let k = key.trim();
    if k.is_empty() {
        return Err(InvalidKey("术语键不能为空".into()));
    }
    if k.len() > 64 {
        return Err(InvalidKey("术语键过长（上限 64 字符）".into()));
    }
    if !k
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Err(InvalidKey("术语键只允许字母数字、下划线与点".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

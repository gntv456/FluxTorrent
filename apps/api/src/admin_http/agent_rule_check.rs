//! 0351 agent_rules 写入口请求体与校验（自 agent_rules.rs 拆出，
//! 300 行门禁）。三件套：正则预校验（原有）+ 版本区间形状/自洽 + priority。

use serde::Deserialize;

use crate::errors::{DomainError, DomainResult};

#[derive(Deserialize)]
pub(crate) struct AgentRuleReq {
    pub(crate) mode: String,
    pub(crate) pattern: String,
    #[serde(default)]
    pub(crate) note: Option<String>,
    /// 0351：版本区间（点分数字，空 = 不限）与优先级（0..=1000，大者先判）
    #[serde(default)]
    pub(crate) version_min: String,
    #[serde(default)]
    pub(crate) version_max: String,
    #[serde(default)]
    pub(crate) priority: i32,
}

/// pattern 会被 tracker 编译成正则（`guard_refresh.rs` 里 `Regex::new`），
/// 编译失败就在加载阶段被跳过——站点侧看起来「已配置」，实际恒不生效。
/// 所以在写入口就判掉：允许合法正则，拒绝非法语法与空模式。
pub(crate) fn check_pattern(p: &str) -> DomainResult<()> {
    // 详情（编译器报错）走日志，不进用户可见串：format! 拼的句子无法按原句查表
    regex::Regex::new(p)
        .map_err(|e| {
            tracing::warn!(%e, %p, "agent_rules pattern 不是合法正则");
            DomainError::Validation("pattern 需是合法正则表达式".into())
        })
        .map(|_| ())
}

/// 0351：版本区间形状校验——只接受点分数字（1-3 段，如 `4.6`/`5.0.1`）
/// 或空串（= 不限）。存储与 tracker 侧比较都按 `(u64,u64,u64)` 数字段
/// 处理，写入口挡掉 `4.x`、`v4.6`、`4.6.0.1` 这类到加载期才静默失效的串。
fn check_version_shape(v: &str, field: &str) -> DomainResult<()> {
    if v.is_empty() {
        return Ok(());
    }
    let parts: Vec<&str> = v.split('.').collect();
    let shape_ok = (1..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if shape_ok {
        Ok(())
    } else {
        Err(DomainError::Validation(format!(
            "{field} 需为点分数字（如 4.6 / 5.0.1）或留空"
        )))
    }
}

/// 点分数字串 → [u64;3]（缺段补 0；写入口已挡畸形，这里兜底 0）。
pub(crate) fn ver_tuple(s: &str) -> [u64; 3] {
    let mut seg = [0u64; 3];
    for (i, p) in s.split('.').take(3).enumerate() {
        seg[i] = p.parse().unwrap_or(0);
    }
    seg
}

/// `AgentRuleReq` 三新字段的统一校验：形状 + 区间自洽
/// （min <= max，按数字段而非字符串）+ priority 0..=1000。
pub(crate) fn check_version_priority(r: &AgentRuleReq) -> DomainResult<()> {
    let mn = r.version_min.trim();
    let mx = r.version_max.trim();
    for (v, f) in [(mn, "version_min"), (mx, "version_max")] {
        check_version_shape(v, f)?;
    }
    if !mn.is_empty() && !mx.is_empty() && ver_tuple(mn) > ver_tuple(mx) {
        return Err(DomainError::Validation(
            "version_min 不能大于 version_max".into(),
        ));
    }
    if !(0..=1000).contains(&r.priority) {
        return Err(DomainError::Validation("priority 取值 0-1000".into()));
    }
    Ok(())
}

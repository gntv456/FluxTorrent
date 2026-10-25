//! 站点策略的归一与版本化（公开口径的单一事实源）。
//!
//! 背景：H&R 的做种时长/期限/预警/违规阈值此前由 worker、`repo::hr`
//! 与两个前端页面各读各的键、各写各的默认，展示侧甚至写死过 14 天，
//! 而 worker 会按种子政策与等级折扣执法——同一名会员能看到两种承诺。
//!
//! 这里把「归一后的生效值」收成一个纯值对象：缺行、垃圾值、越界值
//! 的每个缺省与夹取都与执法点逐字对齐；`version` 对生效值做摘要，
//! 改配置可被跨页对账。**不公开**反作弊权重与分档阈值。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

/// 一行站点设定：键、原值、最后写入时刻。
pub type PolicyRow = (String, String, DateTime<Utc>);

/// 会员义务相关站点口径的**生效值**（已套默认与夹取）。
///
/// 每个字段都要与真正的执法点同口径，否则公开页承诺的与 worker
/// 实际执行的会变成两套数：
///   · seed_hours / days → `repo::hr::{hours,days}_setting`
///   · warn_enable / base_days / max_shift_days → `jobs/hr_warn_ladder`
///   · violation_limit → `jobs/hr::hr_punish`
///   · prewarn_hours → `jobs/hr`（0＝关闭预警段）
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct HrPolicy {
    pub seed_hours: i32,
    pub days: i32,
    pub warn_enable: bool,
    pub prewarn_hours: i32,
    pub violation_limit: i32,
    pub warn_base_days: i32,
    pub warn_max_shift_days: i32,
}

impl Default for HrPolicy {
    fn default() -> Self {
        Self {
            seed_hours: 48,
            days: 14,
            warn_enable: true,
            prewarn_hours: 0,
            violation_limit: 3,
            warn_base_days: 7,
            warn_max_shift_days: 365,
        }
    }
}

/// 公开策略快照：同一份值同时喂 `/system/policies` 与 `/me/hr`。
///
/// `version` 对**归一后的生效值**做 SHA-256（取前 16 位十六进制）：
/// 改任意一档 → 版本必变；把值改回原样 → 版本也回到原值。
/// 它标识配置内容，不是编辑次数，可以直接用于跨页对账。
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PolicySnapshot {
    pub schema: i32,
    pub version: String,
    /// 参与口径的键里最近一次写入时刻；键全缺时为 None。
    pub updated_at: Option<DateTime<Utc>>,
    pub hr: HrPolicy,
    /// 风险分衰减半衰期（天），与 worker `risk_score` 同口径。
    /// 只公开衰减，不公开权重与分档阈值——那等于把判据交给对手。
    pub risk_decay_days: f64,
}

impl PolicySnapshot {
    pub const SCHEMA: i32 = 1;

    /// 纯函数：缺行 / 垃圾值 / 越界值都归一到与执法点相同的生效值。
    pub fn from_rows(rows: &[PolicyRow]) -> Self {
        let map: HashMap<&str, &str> = rows
            .iter()
            .map(|(k, v, _)| (k.as_str(), v.as_str()))
            .collect();
        let hr = HrPolicy {
            // worker 走 SQL `value::bigint`（接受首尾空白），然后 clamp
            seed_hours: int64_sql(&map, "hr_hours", 48).clamp(1, 24 * 365)
                as i32,
            days: int64_sql(&map, "hr_days", 14).clamp(1, 365) as i32,
            warn_enable: warn_enabled(&map),
            // worker 把 bigint 直接 `as i32` 交给 prewarn_run；≤0 即关闭
            prewarn_hours: int64_sql(&map, "hr_prewarn_hours", 0)
                .clamp(0, i32::MAX as i64) as i32,
            // worker hr_punish: 原值 parse::<i64>()（不 trim）后 clamp
            violation_limit: map
                .get("hr_violation_limit")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(3)
                .clamp(1, 100) as i32,
            // 阶梯作业用 v.parse::<i32>() 原样解析（不 trim），
            // 且只有下限（.max(1) / .max(0)），上限不另加夹取
            warn_base_days: int32_raw(&map, "hr_warn_base_days", 7).max(1),
            warn_max_shift_days: int32_raw(&map, "hr_warn_max_shift_days", 365)
                .max(0),
        };
        let risk_decay_days = risk_decay(&map);
        let updated_at = rows.iter().map(|(_, _, t)| *t).max();
        Self {
            schema: Self::SCHEMA,
            version: version_of(&hr, risk_decay_days),
            updated_at,
            hr,
            risk_decay_days,
        }
    }
}

/// SQL `value::bigint` 的等价解析：接受首尾空白；垃圾值回落缺省。
fn int64_sql(map: &HashMap<&str, &str>, key: &str, default: i64) -> i64 {
    map.get(key)
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

/// worker 直接 `value.parse::<i32>()` 的键：不 trim，失败即缺省。
fn int32_raw(map: &HashMap<&str, &str>, key: &str, default: i32) -> i32 {
    map.get(key)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default)
}

/// 与 worker 逐字同口径：只有整值 "0" / "no" 关闭（不 trim），其余
/// （含 "false"、拼错、空串）都视为开启。不要在这里"顺手"支持 false——
/// 那会让公开页说已关闭、worker 仍在执行。
fn warn_enabled(map: &HashMap<&str, &str>) -> bool {
    map.get("hr_warn_enable")
        .map(|v| *v != "0" && *v != "no")
        .unwrap_or(true)
}

fn risk_decay(map: &HashMap<&str, &str>) -> f64 {
    let raw = map
        .get("risk_decay_days")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(14.0);
    // worker 对 ±inf 会被 clamp 到边界；只有 NaN 无处安放（serde_json
    // 输出 null 并破坏版本串），回落缺省
    let raw = if raw.is_nan() { 14.0 } else { raw };
    raw.clamp(0.5, 3_650.0)
}

fn version_of(hr: &HrPolicy, risk_decay_days: f64) -> String {
    let mut lines = vec![
        format!("schema={}", PolicySnapshot::SCHEMA),
        format!("hr.seed_hours={}", hr.seed_hours),
        format!("hr.days={}", hr.days),
        format!("hr.warn_enable={}", hr.warn_enable),
        format!("hr.prewarn_hours={}", hr.prewarn_hours),
        format!("hr.violation_limit={}", hr.violation_limit),
        format!("hr.warn_base_days={}", hr.warn_base_days),
        format!("hr.warn_max_shift_days={}", hr.warn_max_shift_days),
        format!("risk.decay_days={}", risk_decay_days),
    ];
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn rows(pairs: &[(&str, &str)]) -> Vec<PolicyRow> {
        let at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string(), at))
            .collect()
    }

    #[test]
    fn missing_rows_yield_documented_defaults() {
        let p = PolicySnapshot::from_rows(&[]);
        assert_eq!(p.hr, HrPolicy::default());
        assert_eq!(p.risk_decay_days, 14.0);
        assert_eq!(p.schema, 1);
        assert!(p.updated_at.is_none());
        assert_eq!(p.version.len(), 16);
    }

    #[test]
    fn version_ignores_row_order_and_timestamps() {
        let a = rows(&[("hr_hours", "120"), ("hr_days", "20")]);
        let mut b = rows(&[("hr_days", "20"), ("hr_hours", "120")]);
        b[0].2 = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(
            PolicySnapshot::from_rows(&a).version,
            PolicySnapshot::from_rows(&b).version,
        );
    }

    #[test]
    fn version_changes_with_effective_value() {
        let base = PolicySnapshot::from_rows(&rows(&[("hr_hours", "48")]));
        let changed = PolicySnapshot::from_rows(&rows(&[("hr_hours", "72")]));
        assert_ne!(base.version, changed.version);
    }

    #[test]
    fn invalid_values_fall_back_to_clamps() {
        let p = PolicySnapshot::from_rows(&rows(&[
            ("hr_hours", "abc"),
            ("hr_days", "0"),
            ("hr_violation_limit", "999"),
            ("hr_warn_enable", "no"),
            ("risk_decay_days", "NaN"),
        ]));
        assert_eq!(p.hr.seed_hours, 48);
        assert_eq!(p.hr.days, 1);
        assert_eq!(p.hr.violation_limit, 100);
        assert!(!p.hr.warn_enable);
        assert_eq!(p.risk_decay_days, 14.0);
    }

    #[test]
    fn bigint_keys_parse_like_worker_casts() {
        let p = PolicySnapshot::from_rows(&rows(&[
            ("hr_hours", "9999999999"),
            ("hr_days", "9999999999"),
            ("hr_prewarn_hours", "9000"),
            ("hr_violation_limit", "9999999999"),
        ]));
        assert_eq!(p.hr.seed_hours, 8_760);
        assert_eq!(p.hr.days, 365);
        // prewarn 在执法侧没有上限夹取（0 才是关闭）
        assert_eq!(p.hr.prewarn_hours, 9_000);
        assert_eq!(p.hr.violation_limit, 100);
    }

    #[test]
    fn raw_parsed_keys_do_not_trim() {
        let p = PolicySnapshot::from_rows(&rows(&[
            ("hr_violation_limit", " 5 "),
            ("hr_warn_base_days", " 30 "),
            ("hr_warn_max_shift_days", " 90 "),
            ("hr_warn_enable", " 0 "),
        ]));
        assert_eq!(p.hr.violation_limit, 3);
        assert_eq!(p.hr.warn_base_days, 7);
        assert_eq!(p.hr.warn_max_shift_days, 365);
        assert!(p.hr.warn_enable);
    }

    #[test]
    fn infinite_decay_clamps_to_bounds() {
        let hi =
            PolicySnapshot::from_rows(&rows(&[("risk_decay_days", "inf")]));
        assert_eq!(hi.risk_decay_days, 3_650.0);
        let lo =
            PolicySnapshot::from_rows(&rows(&[("risk_decay_days", "-inf")]));
        assert_eq!(lo.risk_decay_days, 0.5);
    }

    #[test]
    fn warn_enable_matches_worker_semantics() {
        let on =
            PolicySnapshot::from_rows(&rows(&[("hr_warn_enable", "false")]));
        assert!(on.hr.warn_enable);
        let off = PolicySnapshot::from_rows(&rows(&[("hr_warn_enable", "0")]));
        assert!(!off.hr.warn_enable);
    }

    #[test]
    fn updated_at_is_latest_row_timestamp() {
        let mut r = rows(&[("hr_hours", "48"), ("hr_days", "14")]);
        r[1].2 = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let p = PolicySnapshot::from_rows(&r);
        assert_eq!(p.updated_at, Some(r[1].2));
    }
}

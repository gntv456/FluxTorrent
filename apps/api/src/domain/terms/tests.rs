//! 术语治理的单测（覆盖率不能单独看——退役引用会让它假绿）。

use super::*;

fn inp(total: i64, active: i64, dep: i64, ret: i64, rule: i64) -> AuditInput {
    AuditInput {
        total, active, deprecated: dep, retired: ret, with_rule: rule,
    }
}

/// 覆盖率 = 有规则键 / 在用键。
#[test]
fn coverage_is_rule_over_active() {
    let a = audit(&inp(100, 80, 10, 10, 64), 0);
    assert_eq!(a.coverage_pct, 80.0);
}

/// 【关键】一个在用键都没登记 → 覆盖率 0，而非 100。
#[test]
fn zero_active_means_zero_coverage_not_full() {
    let a = audit(&inp(50, 0, 0, 50, 0), 0);
    assert_eq!(
        a.coverage_pct, 0.0,
        "没登记任何在用键时算成 100% 正好和事实相反"
    );
}

/// 已退役键不计入分母（它们本就不该被替换）。
#[test]
fn retired_excluded_from_denominator() {
    // 50 个在用 + 50 个退役，只有 25 个在用键有规则 → 50%，不是 25%
    let a = audit(&inp(100, 50, 0, 50, 25), 0);
    assert_eq!(a.coverage_pct, 50.0);
}

/// 覆盖率保留一位小数。
#[test]
fn coverage_rounds_to_one_decimal() {
    let a = audit(&inp(3, 3, 0, 0, 1), 0);
    assert_eq!(a.coverage_pct, 33.3);
}

/// 【关键】零退役引用 + 高覆盖 = 真绿。
#[test]
fn healthy_requires_no_retired_refs() {
    assert!(audit(&inp(100, 90, 5, 5, 90), 0).healthy);
}

/// 【关键】覆盖率高但引用着退役键 = 假绿，不健康。
#[test]
fn high_coverage_with_retired_refs_is_unhealthy() {
    // 90 个在用键里 81 个有规则 = 90% 覆盖（够过了 80 门槛），
    // 但仍引用着 3 个退役键 → 不健康。
    let a = audit(&inp(100, 90, 5, 5, 81), 3);
    assert_eq!(a.coverage_pct, 90.0);
    assert!(!a.healthy, "覆盖率好看但有退役引用是典型的假绿");
}

/// 覆盖不足（<80%）不健康，即便零退役引用。
#[test]
fn low_coverage_is_unhealthy() {
    let a = audit(&inp(100, 100, 0, 0, 50), 0);
    assert_eq!(a.coverage_pct, 50.0);
    assert!(!a.healthy);
}

/// 状态解析：未知值回落 Active（而不是更宽松的态）。
#[test]
fn status_parse_is_conservative() {
    assert_eq!(Status::parse("active"), Status::Active);
    assert_eq!(Status::parse("deprecated"), Status::Deprecated);
    assert_eq!(Status::parse("retired"), Status::Retired);
    assert_eq!(Status::parse("garbage"), Status::Active);
}

/// 退役键不得被新代码引用；废弃键仍可。
#[test]
fn retired_blocks_reference() {
    assert!(Status::Active.allows_reference());
    assert!(Status::Deprecated.allows_reference());
    assert!(!Status::Retired.allows_reference());
}

/// 退役是单向门，但保留撤回通道。
#[test]
fn retirement_is_one_way_with_rollback() {
    assert!(can_transition(Status::Deprecated, Status::Retired));
    assert!(can_transition(Status::Retired, Status::Active),
            "撤回误操作必须留通道");
    // 刻意退役的键不能直接跳去别的态，必须先回 active
    assert!(!can_transition(Status::Retired, Status::Deprecated));
    // active → retired 也要走 deprecated 一步
    assert!(!can_transition(Status::Active, Status::Retired));
    // 同态是空操作，恒允许
    assert!(can_transition(Status::Active, Status::Active));
}

/// 键名校验：字符集受限（键会进 URL/Redis key/前端字典）。
#[test]
fn validate_key_charset() {
    assert!(validate_key("site_name").is_ok());
    assert!(validate_key("forum.post_count").is_ok());
    assert!(validate_key("").is_err());
    assert!(validate_key("   ").is_err());
    assert!(validate_key("有中文").is_err());
    assert!(validate_key("has space").is_err());
    assert!(validate_key("has/slash").is_err());
    assert!(validate_key("has-dash").is_err());
}

/// 键长上限。
#[test]
fn validate_key_length() {
    assert!(validate_key(&"a".repeat(64)).is_ok());
    assert!(validate_key(&"a".repeat(65)).is_err());
}

/// 键名两端空白允许（trim 后校验），但不该存成带空白的键。
#[test]
fn validate_key_trims() {
    assert!(validate_key("  site_name  ").is_ok());
}

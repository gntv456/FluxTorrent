//! H&R 分档 + 做种减免的单测。
//!
//! 最要紧的两条边界用红测钉死：减免只降门槛不降责任（永不归零）、
//! 阶梯不叠加（否则「做种多」变成指数收益）。

use super::*;

fn ladder() -> Vec<Relief> {
    vec![
        Relief { extra_hours: 100, relief_pct: 90 },
        Relief { extra_hours: 500, relief_pct: 80 },
        Relief { extra_hours: 2000, relief_pct: 70 },
        Relief { extra_hours: 5000, relief_pct: 60 },
    ]
}

/// 无分档 = 按种子原值（不减免）。
#[test]
fn no_tier_means_no_change() {
    let r = requirement(100, 14, None, &ladder(), 0, 1);
    assert_eq!(r.seed_hours, 100);
    assert_eq!(r.days, 14);
    assert_eq!(r.relief_tier, None);
}

/// 严苛档（150%）让门槛更高。
#[test]
fn strict_tier_raises_requirement() {
    let t = Tier { seed_hours_pct: 150, days_pct: 80 };
    let r = requirement(100, 14, Some(t), &[], 0, 1);
    assert_eq!(r.seed_hours, 150);
    assert_eq!(r.days, 11);
}

/// 减免档（60%）降低门槛。
#[test]
fn reduced_tier_lowers_requirement() {
    let t = Tier { seed_hours_pct: 60, days_pct: 100 };
    let r = requirement(100, 14, Some(t), &[], 0, 1);
    assert_eq!(r.seed_hours, 60);
}

/// 做种减免：额外做种超 100h 触发第一档（只要求 10%）。
#[test]
fn relief_applies_after_threshold() {
    let r = requirement(100, 14, None, &ladder(), 200, 1);
    assert_eq!(r.relief_tier, Some(100));
    assert_eq!(r.seed_hours, 10);
}

/// 阶梯取最靠后一档，不叠加（2000h 用 70%，不是 90%*80%*70%）。
#[test]
fn relief_picks_farthest_tier_not_stacked() {
    let r = requirement(100, 14, None, &ladder(), 2100, 1);
    assert_eq!(r.relief_tier, Some(2000));
    // 叠加的话会是 100*10%*20%*30% = 0.6 → 1（地板）
    // 不叠加是 100 * (100-70)% = 30
    assert_eq!(r.seed_hours, 30);
}

/// 【安全边界】减免永不归零：即使 relief_pct=100 也只降到地板。
#[test]
fn relief_never_zeroes_requirement() {
    let full = vec![Relief { extra_hours: 0, relief_pct: 100 }];
    let r = requirement(100, 14, None, &full, 99999, 1);
    assert_eq!(r.seed_hours, 1, "必须有地板，不能减到 0");
    // 地板为 0 时按 0 走（站长显式关掉地板是允许的，但要能配出来）
    let r0 = requirement(100, 14, None, &full, 99999, 0);
    assert_eq!(r0.seed_hours, 0);
}

/// 【安全边界】分档 0% 也只降到地板。
#[test]
fn zero_tier_still_has_floor() {
    let t = Tier { seed_hours_pct: 0, days_pct: 100 };
    let r = requirement(100, 14, Some(t), &[], 0, 1);
    assert_eq!(r.seed_hours, 1);
}

/// 额外做种为负（seed_hours 配置大于用户实际）时按 0 算，不产生减免。
#[test]
fn negative_extra_means_no_relief() {
    let r = requirement(1000, 14, None, &ladder(), 10, 1);
    assert_eq!(r.relief_tier, None);
    assert_eq!(r.seed_hours, 1000);
}

/// 天数倍率最低 1 天（不会算出 0 天义务）。
#[test]
fn days_never_zero() {
    let t = Tier { seed_hours_pct: 100, days_pct: 10 };
    let r = requirement(100, 1, Some(t), &[], 0, 1);
    assert!(r.days >= 1, "0 天义务没有意义");
}

/// 取档：映射到的取到值。
#[test]
fn tier_of_found() {
    let tiers = [
        ("strict", Tier { seed_hours_pct: 150, days_pct: 80 }),
        ("default", Tier { seed_hours_pct: 100, days_pct: 100 }),
    ];
    let map = vec![(1, "strict".to_string())];
    assert_eq!(tier_of(1, &map, &tiers).unwrap().seed_hours_pct, 150);
}

/// 取档：未映射的等级返回 None（**不减免**，不是「用最松的档」）。
#[test]
fn tier_of_unmapped_is_none() {
    let tiers = [("reduced", Tier { seed_hours_pct: 60, days_pct: 100 })];
    let map = vec![(1, "strict".to_string())];
    assert!(tier_of(9, &map, &tiers).is_none());
}

/// 取档：映射到不存在的档名也返回 None（配置不一致不豁免）。
#[test]
fn tier_of_unknown_tier_name_is_none() {
    let tiers = [("strict", Tier { seed_hours_pct: 150, days_pct: 80 })];
    let map = vec![(1, "typo_here".to_string())];
    assert!(tier_of(1, &map, &tiers).is_none());
}

/// 分档配置校验。
#[test]
fn validate_tier_bounds() {
    assert!(validate_tier(100, 100).is_ok());
    assert!(validate_tier(0, 10).is_ok());
    assert!(validate_tier(400, 400).is_ok());
    assert!(validate_tier(-1, 100).is_err());
    assert!(validate_tier(401, 100).is_err());
    assert!(validate_tier(100, 9).is_err());
    assert!(validate_tier(100, 401).is_err());
}

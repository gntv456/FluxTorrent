//! `ratio_gate` 判据单测（纯函数，无需 DB/Redis）。
//! 每条豁免都单独钉住：少任一条，warn 在真实站点上都会立刻变成对无辜者的 block。

use super::*;

fn g(mode: i8, class_min: f64, site_min: f64) -> Gate {
    Gate {
        mode,
        class_min,
        site_min,
        ceiling: 1.0,
        grace_days: 7,
        class_age_days: 0,
    }
}

fn w(up: i64, down: i64) -> Who {
    Who {
        uploaded: up,
        downloaded: down,
        age_days: 30,
        in_watch: false,
        staff: false,
    }
}

#[test]
fn threshold_is_the_larger_and_gets_clamped() {
    // 站点 0.4 vs 等级 0.8 ⇒ 取 0.8
    let (t, c) = threshold(&g(WARN, 0.8, 0.4));
    assert!((t - 0.8).abs() < 1e-9 && !c);
    // 现网的 ratiolimit=6 必须被截到 1.0 并报出「截断了」
    let (t, c) = threshold(&g(BLOCK, 0.0, 6.0));
    assert!((t - 1.0).abs() < 1e-9, "{t}");
    assert!(c, "超上界必须报截断，不能静默按 6 执行");
    // 两边都没设 ⇒ 不设闸
    let (t, _) = threshold(&g(BLOCK, 0.0, 0.0));
    assert_eq!(t, 0.0);
}

#[test]
fn off_and_staff_never_block() {
    assert_eq!(
        decide(&g(OFF, 0.0, 0.9), &w(0, 100 * 1024)),
        Decision::Allow
    );
    let st = Who {
        staff: true,
        ..w(0, 100 * 1024)
    };
    assert_eq!(decide(&g(BLOCK, 0.0, 0.9), &st), Decision::Allow);
}

#[test]
fn low_ratio_warns_or_blocks_by_mode() {
    let low = w(1024, 100 * 1024); // 比率 0.01
    assert_eq!(decide(&g(WARN, 0.0, 0.4), &low), Decision::Warn);
    assert_eq!(decide(&g(BLOCK, 0.0, 0.4), &low), Decision::Block);
}

#[test]
fn at_or_above_threshold_passes() {
    // 正好踩线 = 达标（这是准入门槛，不是「超过即罚」的闸）
    let exact = w(40 * 1024, 100 * 1024);
    assert_eq!(decide(&g(BLOCK, 0.4, 0.0), &exact), Decision::Allow);
    let above = w(41 * 1024, 100 * 1024);
    assert_eq!(decide(&g(BLOCK, 0.4, 0.0), &above), Decision::Allow);
}

/// 三条豁免，一条都不能少：新号、零下载、已在观察期。
/// 少任何一条，「warn」在真实站点上都会立刻变成对无辜者的 block。
#[test]
fn exemptions_hold_the_gate_from_hurting_the_blameless() {
    // ① 零下载：比率为 0 是「没有数据」不是「比率很差」
    let fresh = w(0, 0);
    assert_eq!(decide(&g(BLOCK, 0.0, 0.4), &fresh), Decision::Allow);
    // ② 新人：注册 3 天，宽限 7 天
    let newf = Who {
        age_days: 3,
        ..w(0, 100 * 1024)
    };
    assert_eq!(decide(&g(BLOCK, 0.0, 0.4), &newf), Decision::Allow);
    // 等级自己要求 30 天 ⇒ 与站点宽限取较大者
    let g30 = Gate {
        class_age_days: 30,
        ..g(BLOCK, 0.0, 0.4)
    };
    assert_eq!(decide(&g30, &newf), Decision::Allow);
    // ③ 观察期内：ratio_watch 已经给了期限，闸门不重复处罚
    let watch = Who {
        in_watch: true,
        ..w(0, 100 * 1024)
    };
    assert_eq!(decide(&g(BLOCK, 0.0, 0.4), &watch), Decision::Allow);
    // 宽限到 0 天时新人不再被豁免（站长显式要「不留宽限」时得能表达）
    let g0 = Gate {
        grace_days: 0,
        ..g(BLOCK, 0.0, 0.4)
    };
    assert_eq!(decide(&g0, &newf), Decision::Block);
}

#[test]
fn unknown_age_is_not_treated_as_exempt_forever() {
    // age_days<0（时间戳缺失）⇒ 不据「新人」豁免，但也不误判为老人之外的
    // 别的口子：仍按比率判
    let u = Who {
        age_days: -1,
        ..w(0, 100 * 1024)
    };
    assert_eq!(decide(&g(BLOCK, 0.0, 0.4), &u), Decision::Block);
}

#[test]
fn mode_text_maps_and_defaults_to_warn() {
    assert_eq!(mode_of("off"), OFF);
    assert_eq!(mode_of("block"), BLOCK);
    // 缺省/写错一律 warn：闸门不会因为一个错别字就变成 block 或彻底关掉
    assert_eq!(mode_of("Block"), WARN);
    assert_eq!(mode_of(""), WARN);
    assert_eq!(mode_of("nonsense"), WARN);
}

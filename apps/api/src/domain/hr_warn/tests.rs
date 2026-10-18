//! H&R 警告顺延的单测。
//!
//! 最要紧两条：顺延要有上限（否则等于无期），恢复要看**全部**警告
//! 而非最早那条。

use super::*;
use chrono::TimeZone;

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc.with_ymd_and_hms(2026, 10, 16, 0, 0, 0).unwrap()
}

fn inp(open: i32, earliest: Option<i32>, base: i32, cap: i32) -> WarnInput {
    WarnInput {
        open_warnings: open,
        earliest_expiry: earliest
            .map(|d| now() + chrono::Duration::days(d as i64)),
        base_days: base,
        max_shift_days: cap,
        now: now(),
    }
}

/// 阶梯按未解决警告数递增。
#[test]
fn ladder_escalates() {
    assert_eq!(step_for(0).1.days, 7);
    assert_eq!(step_for(1).1.days, 14);
    assert_eq!(step_for(2).1.days, 30);
    assert_eq!(step_for(3).1.days, 90);
}

/// 超出阶梯停在最严档，不越界。
#[test]
fn ladder_clamps_at_strictest() {
    let (idx, s) = step_for(99);
    assert_eq!(idx, LADDER.len() - 1);
    assert_eq!(s.days, 90);
}

/// 阶梯的处置逐档加严。
#[test]
fn ladder_restrictions_escalate() {
    assert!(!step_for(0).1.restrict_download);
    assert!(step_for(1).1.restrict_download);
    assert!(!step_for(1).1.block_upload);
    assert!(step_for(2).1.block_upload);
    assert!(!step_for(2).1.block_invite);
    assert!(step_for(3).1.block_invite);
}

/// 第一次警告（无在先）从今天起算，不顺延。
#[test]
fn first_warning_not_shifted() {
    let d = decide(&inp(0, None, 7, 0));
    assert_eq!(d.shifted_days, 0);
    assert_eq!(d.expires_at, now() + chrono::Duration::days(7));
}

/// 【Gazelle 式顺延】第二次警告从第一次的到期时刻往后接。
#[test]
fn second_warning_appends_to_existing() {
    // 在先警告 1 天后到期，新档 14 天 → 到期 = base+14 = now+15
    let d = decide(&inp(1, Some(1), 7, 0));
    assert_eq!(
        d.expires_at,
        now() + chrono::Duration::days(15),
        "应从原到期时刻往后接，而不是从今天重算 14 天"
    );
    assert_eq!(d.shifted_days, 1, "顺延量 = 到期 - 裸警告 = 15 - 14");
}

/// 顺延撞上限时封顶，并报 hit_cap。
#[test]
fn shift_is_capped() {
    // 在先警告 7 天前就到期了（早已解开），新档 30 天：
    // natural = base+30 = now+23 < fresh(now+30) → 抬回 fresh
    // 顺延量 0，撞不到上限。
    let d = decide(&inp(2, Some(-7), 7, 30));
    assert_eq!(d.expires_at, now() + chrono::Duration::days(30));
    assert_eq!(d.shifted_days, 0);
    assert!(!d.hit_cap);

    // 真正撞上限：在先警告还有 10 天到期，新档 30 天 →
    // natural = now+40，fresh = now+30 → 顺延 10 天；上限 5 → 封到 5
    let d2 = decide(&inp(2, Some(10), 7, 5));
    assert_eq!(d2.expires_at, now() + chrono::Duration::days(35));
    assert_eq!(d2.shifted_days, 5);
    assert!(d2.hit_cap);
}

/// 【关键】在先警告到期得比新警告更晚时，新警告跟随旧到期时刻。
///
/// 极端情形：手上压着一条 100 天后才解禁的长警告，此时新违规若按
/// 「现在 + 14 天」算，会**早于**旧警告到期——等于用一条新违规去缩短
/// 旧违规的处罚。所以必须跟随 base。
#[test]
fn new_warning_never_expires_before_existing() {
    let d = decide(&inp(1, Some(100), 7, 0));
    assert_eq!(
        d.expires_at,
        now() + chrono::Duration::days(114),
        "顺延语义是「在先到期之后再加一天数」"
    );
    assert!(d.expires_at > now() + chrono::Duration::days(100));
    assert_eq!(d.shifted_days, 100);
}

/// 同一条到期时刻：新警告正好落在 base 上，不额外加天数。
#[test]
fn identical_expiry_adds_nothing() {
    // 在先警告 now+0（= now），新档 14 天 → base+14 = now+14 = fresh
    let d = decide(&inp(1, Some(0), 7, 0));
    assert_eq!(d.expires_at, now() + chrono::Duration::days(14));
    assert_eq!(d.shifted_days, 0);
}

/// cap=0 表示不封顶（站长显式选择）。
#[test]
fn zero_cap_means_unbounded() {
    // 在先警告 3 天后到期，新档 14 天 → natural = now+17，
    // fresh = now+14 → 顺延 3 天，不封顶
    let d = decide(&inp(1, Some(3), 7, 0));
    assert!(!d.hit_cap);
    assert_eq!(d.shifted_days, 3);
    assert_eq!(d.expires_at, now() + chrono::Duration::days(17));
}

/// 站长配的 base_days 可覆盖阶梯（不被阶梯压低）。
#[test]
fn base_days_can_raise_step() {
    let d = decide(&inp(0, None, 60, 0));
    assert_eq!(d.expires_at, now() + chrono::Duration::days(60));
}

/// 【关键】恢复看全部警告：最早那条到期但还有更晚的 → 不恢复。
#[test]
fn restore_requires_all_expired() {
    let t = now();
    let exps =
        vec![t - chrono::Duration::days(1), t + chrono::Duration::days(5)];
    assert!(
        !should_auto_restore(&exps, t),
        "最早那条已过期但还有 5 天后才到期的，不该放人"
    );
}

/// 全部到期才恢复。
#[test]
fn restore_when_all_expired() {
    let t = now();
    let exps =
        vec![t - chrono::Duration::days(3), t - chrono::Duration::days(1)];
    assert!(should_auto_restore(&exps, t));
}

/// 没有未解决警告 → 直接可恢复。
#[test]
fn restore_with_no_warnings() {
    assert!(should_auto_restore(&[], now()));
}

/// 下载受限：任一未到期即受限（从严）。
#[test]
fn download_restricted_if_any_active() {
    let t = now();
    assert!(is_download_restricted(&[t + chrono::Duration::days(1)], t));
    assert!(is_download_restricted(
        &[t - chrono::Duration::days(1), t + chrono::Duration::days(3)],
        t
    ));
    assert!(!is_download_restricted(&[t - chrono::Duration::days(1)], t));
    assert!(!is_download_restricted(&[], t));
}

/// 顺延上限配置校验。
#[test]
fn validate_max_shift_bounds() {
    assert!(validate_max_shift(0).is_ok());
    assert!(validate_max_shift(365).is_ok());
    assert!(validate_max_shift(3650).is_ok());
    assert!(validate_max_shift(-1).is_err());
    assert!(validate_max_shift(3651).is_err());
}

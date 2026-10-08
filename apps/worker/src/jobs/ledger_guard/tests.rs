//! `ledger_guard` 的锚点判定单测（纯函数，无需 DB）。
//!
//! 拆出来只为一件事：**判据的否定式分支必须自带反例**。批一那个「正常单调
//! 增量也被吞成 0」的缺陷就是没有反例钉着，带着全绿的验收探针上线的。

use super::*;

const GIB: i64 = 1024 * 1024 * 1024;

fn a(up: i64, down: i64) -> (i64, i64) {
    (up, down)
}

/// 情形①：正常单调上报必须真的产生增量。
/// 这条就是 2026-10-08 五轮全站入账停摆的回归钉——批一的判据把这条
/// 落进了「钉死锚点」分支，raw 恒 0，而当时的验收探针只断言「谎报者被拒」，
/// 拒与不记账在同一个断言里长得一模一样（假通过）。
#[test]
fn monotonic_reading_advances_anchor() {
    let got = anchor_for(GIB, GIB / 2, Some(a(0, 0)));
    assert_eq!((got.up, got.down), a(GIB, GIB / 2));
    assert!(!got.refused && !got.reset_up && !got.reset_down);
    // 增量 = 新锚点 − 旧锚点
    assert_eq!((got.up - 0).max(0), GIB);
}

#[test]
fn anchor_never_moves_back_without_near_zero() {
    // 800 GiB → 500 GiB：读数很大却变小 ⇒ 搬基线，锚点钉死
    let got = anchor_for(500 * GIB, 0, Some(a(800 * GIB, 0)));
    assert!(got.refused);
    assert_eq!((got.up, got.down), a(800 * GIB, 0));
    // 钉死后本笔增量恒 0，等真实计数器重新超过旧锚点才继续入账
    assert_eq!((got.up - 800 * GIB).max(0), 0);
}

#[test]
fn near_zero_reset_is_accepted_and_lowers_anchor() {
    // 客户端重启：up 读数回到 12 KiB ⇒ 认可，且标记 reset_up 让 GREATEST 让路
    let got = anchor_for(12 * 1024, 20 * GIB, Some(a(500 * GIB, 20 * GIB)));
    assert!(!got.refused);
    assert_eq!(got.up, 12 * 1024);
    assert!(got.reset_up, "up 侧发生了近零重置，必须允许锚点下调");
    // down 侧这次没降 ⇒ 不该被标记
    assert!(!got.reset_down, "只降了一侧时另一侧不应被当成重置");
    // 本笔增量按 0（新读数比旧锚点小），下一次从新低锚点起算
    assert_eq!((got.up - 500 * GIB).max(0), 0);
    // 两侧都降到近零 ⇒ 两侧都要让路
    let both = anchor_for(12 * 1024, 8 * 1024, Some(a(500 * GIB, 20 * GIB)));
    assert!(both.reset_up && both.reset_down);
}

#[test]
fn first_reading_only_establishes_baseline() {
    // 无基线时谈不上「回退」；调用方另按 raw=0 处理（首报只立基线）
    let got = anchor_for(9000 * GIB, 7000 * GIB, None);
    assert!(!got.refused && !got.reset_up && !got.reset_down);
    assert_eq!((got.up, got.down), a(9000 * GIB, 7000 * GIB));
}

#[test]
fn reset_accept_boundary_is_inclusive() {
    let at = anchor_for(RESET_ACCEPT, 0, Some(a(50 * GIB, 0)));
    assert!(!at.refused && at.reset_up, "= 1 GiB 算近零，认可");
    let over = anchor_for(RESET_ACCEPT + 1, 0, Some(a(50 * GIB, 0)));
    assert!(over.refused, "1 GiB + 1 不算近零，钉死");
}

/// 连续剧本：正常 → 重启 → 重启后继续做种。整条链必须始终能记账，
/// 这是「真用户不被误杀」的最低要求。
#[test]
fn reboot_then_resume_keeps_accruing() {
    let mut last = a(0, 0);
    let mut total = 0;
    // 第一轮：客户端累计 10 GiB
    let g = anchor_for(10 * GIB, 0, Some(last));
    total += (g.up - last.0).max(0);
    last = (g.up, g.down);
    assert_eq!(total, 10 * GIB);
    // 重启：读数回到 8 MiB（近零）⇒ 认可并下调锚点
    let g = anchor_for(8 * 1024 * 1024, 0, Some(last));
    assert!(g.reset_up);
    total += (g.up - last.0).max(0);
    last = (g.up, g.down);
    assert_eq!(total, 10 * GIB, "重启本笔不计，等后续读数");
    // 重启后又上传 3 GiB ⇒ 从新低锚点起算，正常入账
    let g = anchor_for(3 * GIB, 0, Some(last));
    assert!(!g.refused && !g.reset_up);
    total += (g.up - last.0).max(0);
    assert_eq!(total, 13 * GIB - 8 * 1024 * 1024, "追平后照常入账");
}

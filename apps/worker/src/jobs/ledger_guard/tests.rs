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

/// 六轮审计 P0-1 的攻击剧本回归钉：报 1 MiB「重置」+ 120s 后报增量，
/// 在 1 GiB 口径下每个循环可再铸 ≤ 速率钳 × 时窗的量（实测复现）。
/// 收紧到 16 MiB 后，16 MiB ~ 1 GiB 之间的「重置」一律 refused。
#[test]
fn one_gib_reset_is_no_longer_near_zero() {
    // 六轮实测用的正是 1 MiB——真实重启（从 0 起算）合法，但攻击者也用
    // 它反复重置；频次窗（ledger_guard 侧 24h 一次）负责限制真近零的
    // 重复使用，本测试钉住「中等幅度（16MiB~1GiB）的下降直接拒」。
    let got = anchor_for(64 * 1024 * 1024, 0, Some(a(500 * GIB, 0)));
    assert!(got.refused, "64 MiB 的「重置」不是重启，是搬基线");
    assert_eq!((got.up, got.down), a(500 * GIB, 0), "锚点钉死旧值");
}

/// 只归零一侧、另一侧读数仍很大 = 搬基线攻击的最小动作（真实客户端
/// 重启时 up/down 计数器一起归零）⇒ refused。
/// 这是六轮跑红的那个输入——当时它暴露的正是「单侧重置语义未定义」。
#[test]
fn one_sided_reset_with_large_other_side_is_refused() {
    // up 归零但 down 只降到 5 GiB（远超 16 MiB）：另一侧没有跟着近零重启
    let got = anchor_for(0, 5 * GIB, Some(a(500 * GIB, 20 * GIB)));
    assert!(got.refused, "一侧归零另一侧仍很大，不是真实重启的画像");
    assert_eq!((got.up, got.down), a(500 * GIB, 20 * GIB));
}

#[test]
fn near_zero_reset_is_accepted_and_lowers_anchor() {
    // 客户端重启：两侧读数同时回到近零 ⇒ 认可，且标记 reset_* 让 GREATEST 让路
    let got = anchor_for(12 * 1024, 8 * 1024, Some(a(500 * GIB, 20 * GIB)));
    assert!(!got.refused);
    assert_eq!((got.up, got.down), a(12 * 1024, 8 * 1024));
    assert!(got.reset_up, "up 侧发生了近零重置，必须允许锚点下调");
    assert!(got.reset_down, "down 侧也同时归零，两侧都要让路");
    // 本笔增量按 0（新读数比旧锚点小），下一次从新低锚点起算
    assert_eq!((got.up - 500 * GIB).max(0), 0);
    // 纯做种侧的合法形态：up 归零、down 恒 0（从未下降）⇒ 认可
    // （down 侧「没降」不构成否决条件——攻击面是「降了却不近零」）
    let up_only = anchor_for(12 * 1024, 0, Some(a(500 * GIB, 0)));
    assert!(!up_only.refused && up_only.reset_up);
    assert!(!up_only.reset_down, "down 侧没降就不标记");
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
    assert!(!at.refused && at.reset_up, "= 16 MiB 算近零，认可");
    let over = anchor_for(RESET_ACCEPT + 1, 0, Some(a(50 * GIB, 0)));
    assert!(over.refused, "16 MiB + 1 不算近零，钉死");
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

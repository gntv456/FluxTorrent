//! `seeding_gate` 判据单测（纯函数，无需 DB）。
//! 重点是那些**只在缺陷出现时才红**的反例：1 字节佐证不该豁免、
//! 首报自称完成不该计数、off/hard 两档对「实测不可达」的差别。

use super::*;

const GIB: i64 = 1024 * 1024 * 1024;
const SIZE: i64 = 100 * GIB;

fn x(phys_down: i64, corroborated: Option<i64>) -> Evidence {
    Evidence {
        left: 0,
        port: 51413,
        conn: None,
        phys_down,
        corroborated,
        had_baseline: true,
        conn_hard: true,
        completed_event: false,
    }
}

#[test]
fn threshold_is_the_same_yardstick_as_payload() {
    assert_eq!(corr_threshold(SIZE), SIZE * 104 / 1000);
    // 尺寸未知时不留「无规模豁免」的口子
    assert_eq!(corr_threshold(0), GIB);
    assert_eq!(corr_threshold(-5), GIB);
}

/// 五轮实测的那条攻击：1 字节佐证换整条幽灵做种豁免。
#[test]
fn one_byte_vouch_does_not_exempt_ghost() {
    let v = verdict(&x(0, Some(1)), SIZE);
    assert!(!v.seeding, "1 字节佐证不能证明手里有数据");
    assert!(v.ghost_note, "零下载 + 不可信佐证仍应留幽灵痕");
}

#[test]
fn material_vouch_exempts_cross_site_seeder() {
    // 辅种人群：phys_down 恒 0，但站内 leecher 真的从他身上下载够了量
    let good = corr_threshold(SIZE);
    let v = verdict(&x(0, Some(good)), SIZE);
    assert!(v.seeding, "达到可信规模的佐证即物理证据");
    assert!(!v.ghost_note);
}

#[test]
fn tiny_vouch_still_blocked_on_small_torrent() {
    // 小种子也要按比例：1 MiB 种 × 10.4% ≈ 108 KiB
    let small = 1024 * 1024;
    let v = verdict(&x(0, Some(small * 104 / 1000 - 1)), small);
    assert!(!v.seeding);
    let v = verdict(&x(0, Some(small * 104 / 1000)), small);
    assert!(v.seeding);
}

#[test]
fn real_downloader_seeds_without_any_vouch() {
    let v = verdict(&x(SIZE / 2, None), SIZE);
    assert!(v.seeding && !v.ghost_note);
}

#[test]
fn completed_needs_baseline_and_payload() {
    let mut c = x(SIZE / 2, None);
    c.completed_event = true;
    // 首报即自称完成：UNIT3D 口径直接拒
    c.had_baseline = false;
    assert!(!verdict(&c, SIZE).completed, "没有既有 peer 行就不算完成");
    // 有基线 + 有物理下载量 ⇒ 算
    c.had_baseline = true;
    assert!(verdict(&c, SIZE).completed);
    // 只下了 1% 的「完成」不算
    c.phys_down = SIZE / 100;
    assert!(!verdict(&c, SIZE).completed, "未达比例下限的完成不算");
    // 没发 completed 事件时，即使一切条件满足也不算
    c.phys_down = SIZE;
    c.completed_event = false;
    assert!(!verdict(&c, SIZE).completed);
}

#[test]
fn probed_unreachable_never_counts_as_seeding() {
    let mut c = x(SIZE, Some(9 * SIZE));
    c.conn = Some(0);
    assert!(
        !verdict(&c, SIZE).seeding,
        "回连实测不可达时再多的量也不算在种"
    );
    // 未测（None）不据此判作弊
    c.conn = None;
    assert!(verdict(&c, SIZE).seeding);
}

/// 缺省档（off）：实测不可达**不**否决在种——NAT 后没有映射入站端口
/// 的真做种者是常态，主流三家都只把可达性当信息位。留痕走
/// `snatches.connectable` 那一列，不吃掉状态。
#[test]
fn gate_off_keeps_unreachable_peer_seeding() {
    let mut hard = x(0, Some(corr_threshold(SIZE)));
    hard.conn = Some(0);
    hard.conn_hard = true;
    assert!(!verdict(&hard, SIZE).seeding, "hard 档仍按旧口径否决");
    hard.conn_hard = false;
    assert!(
        verdict(&hard, SIZE).seeding,
        "不否决档（缺省）下不可达也计在种（留痕由 snatches.connectable 承担）"
    );
}

#[test]
fn port_zero_or_partial_left_is_not_seeding() {
    let mut c = x(SIZE, None);
    c.port = 0;
    assert!(!verdict(&c, SIZE).seeding);
    assert!(
        !verdict(&c, SIZE).ghost_note,
        "port=0 不是「声称在种」的现场"
    );
    c.port = 51413;
    c.left = 1;
    assert!(!verdict(&c, SIZE).seeding, "left>0 是在下载不是做种");
}

#[test]
fn corroboration_is_bounded_by_own_download() {
    // leecher 自己只被记账下载了 3 GiB，却想为 A 背书 100 GiB
    assert_eq!(corr_room(100 * GIB, 3 * GIB, 0), 3 * GIB);
    // 已经背过 2 GiB ⇒ 只剩 1 GiB 的额度
    assert_eq!(corr_room(100 * GIB, 3 * GIB, 2 * GIB), 1 * GIB);
    // 背超了不再给额度，但也不许把已有佐证往回冲
    assert_eq!(corr_room(10 * GIB, 3 * GIB, 5 * GIB), 0);
    // 正常小额如实记账
    assert_eq!(corr_room(GIB, 3 * GIB, 0), GIB);
    assert_eq!(corr_room(0, 3 * GIB, 0), 0);
    assert_eq!(corr_room(-1, 3 * GIB, 0), 0);
}

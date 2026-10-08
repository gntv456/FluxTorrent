//! 幽灵做种 / 完成判定与交叉佐证的物理上界（审计 2026-10-08 五轮自
//! process_event.rs 拆出，同受 300 行门禁约束）。
//!
//! 拆出来的真正理由不是行数：这两条判据自 2026-10-07 起一直在改口径，
//! 每次改都只在 process_event.rs 里加一段注释，判据本身没有单测——
//! 于是批一那个「正常增量也被吞成 0」的锚点缺陷带着 22/22 全绿的验收
//! 探针上线了。判据进纯函数、证据取数留在调用方，才有可测性。

/// 佐证豁免的最低可信量（字节）：种子大小的 10.4%，与 `phys_down` 的
/// 比例下限、H&R buffer 同一把尺子。
///
/// 为什么不能是「> 0 即豁免」：2026-10-08 的辅种豁免把判据写成了
/// `upload_corroborated.bytes > 0`，而佐证量本身是 leecher 自报的
/// （见 [`corr_room`]）。两者叠加 = 任何一个别的账号发一条
/// `xreport=<你的 peer_id>:1`，幽灵做种与 `completed` 两条不变量同时失效，
/// 攻击成本一条 announce。豁免要成立，佐证必须**既受物理上界约束、
/// 又达到可信规模**。
pub(crate) fn corr_threshold(torrent_size: i64) -> i64 {
    if torrent_size > 0 {
        torrent_size.saturating_mul(104) / 1000
    } else {
        // 尺寸未知（老种子/畸形元数据）：退回 1 GiB 的绝对下限，
        // 不给「无规模约束的豁免」留口子
        1024 * 1024 * 1024
    }
}

/// 在种 / 完成判定的输入（全部是**站点侧**量，不接受客户端自报累计读数）：
///
///   · `left/port` —— announce 声明；`port=0` 不可能提供上传，直接不是在种
///   · `conn` —— tracker 回连 + BT 握手实测：`Some(0)`=实测不可达（一票否决），
///     `None`=本轮未抽样，不据此判作弊
///   · `phys_down` —— credited 下载（倍率前、速率钳后）∪ 历史 credited
///   · `corroborated` —— 已被 [`corr_room`] 约束过的佐证总量
///   · `had_baseline` —— 此前是否已有 peer 行（UNIT3D「首报即自称完成不算」）
#[derive(Clone, Copy, Debug)]
pub(crate) struct Evidence {
    pub(crate) left: i64,
    pub(crate) port: u16,
    pub(crate) conn: Option<i16>,
    pub(crate) phys_down: i64,
    pub(crate) corroborated: Option<i64>,
    pub(crate) had_baseline: bool,
    /// 本次是不是 `event=completed`
    pub(crate) completed_event: bool,
}

/// 判据结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Verdict {
    pub(crate) seeding: bool,
    pub(crate) completed: bool,
    /// 幽灵画像留痕：left=0 却既没下过、也没被可信佐证
    pub(crate) ghost_note: bool,
}

/// 在种 / 完成判定。
///
/// `seeding`（四条件）：left=0 且 port>0 且实测未被判不可达，且有物理下载量
/// **或**可信佐证；比例下限同样两条路（辅种人群的数据来自外站，phys_down
/// 恒 0，佐证就是他们的物理量）。
/// `completed`：真发 completed 事件 + left=0 + 上面两条证据 + 已有 peer 行。
pub(crate) fn verdict(x: &Evidence, torrent_size: i64) -> Verdict {
    let has_payload = x.phys_down > 0;
    let vouched_ok = x
        .corroborated
        .is_some_and(|c| c >= corr_threshold(torrent_size));
    // 比例下限与 H&R buffer 同口径（size×10.4%），整数运算避免浮点误差
    let ratio_ok = torrent_size <= 0
        || x.phys_down
            .saturating_mul(1000)
            .ge(&torrent_size.saturating_mul(104));
    let proven = has_payload || vouched_ok;
    let covered = ratio_ok || vouched_ok;
    let seeding =
        x.left == 0 && x.port > 0 && x.conn != Some(0) && proven && covered;
    let completed =
        x.completed_event && x.left == 0 && proven && covered && x.had_baseline;
    // 幽灵签名：既没真下过、也没达到可信规模的佐证
    let ghost_note = x.left == 0 && x.port > 0 && !has_payload && !vouched_ok;
    Verdict {
        seeding,
        completed,
        ghost_note,
    }
}

/// 一笔交叉上报最多能为这位上传者背书多少字节。
///
/// **不变式**：一个 leecher 能佐证的总量 ≤ 它自己在这颗种上被记账的
/// 物理下载量。理由：它只能为「自己真的收到过的字节」作证。
/// 没有这条上界时，`upload_corroborated` 就是一台铸币机——B 账号每
/// announce 一次报 `xreport=<A 的 peer_id>:10 GiB`（10 GiB 以下的
/// 单笔过去完全不设限），A 的自报上传量随即被「独立第三方确认」，
/// 而 process_event 的正是要看这个确认量来决定还能入账多少。
///
/// `already` 是该 leecher 在这颗种上**本轮之前**已背书过的总量
/// （跨所有上传者求和——他从 A 与 C 拿到的字节加起来不可能超过他下载的）；
/// `leecher_credited` 是站点侧记账的该 leecher 下载量。
pub(crate) fn corr_room(
    claimed: i64,
    leecher_credited: i64,
    already: i64,
) -> i64 {
    if claimed <= 0 {
        return 0;
    }
    (leecher_credited - already).max(0).min(claimed)
}

#[cfg(test)]
mod tests {
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
}

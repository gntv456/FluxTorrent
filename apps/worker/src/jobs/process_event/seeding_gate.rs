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
    /// 回连实测不可达是否**否决**在种（仅 `connectable_gate=hard` 为 true）。
    /// 默认 off：不可达只留在 `snatches.connectable` 这一列供版主筛，
    /// 不吃掉「在种」状态，也不另写 cheat_events——NAT 后没有映射入站端口的
    /// 真做种者是常态（主流三家都只把可达性当信息位，见本轮报告 §八-7），
    /// 把它记成作弊事件会把管理组信箱刷满并误伤真人。
    pub(crate) conn_hard: bool,
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
    // 回连实测不可达只在 hard 档一票否决（缺省不否决，见 `conn_gate_hard`）。
    let reachable = !x.conn_hard || x.conn != Some(0);
    let seeding = x.left == 0 && x.port > 0 && reachable && proven && covered;
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

/// `connectable_gate` 档位：**只有两档，不做三档假选择**。
/// `off`（缺省）= 实测不可达不否决在种；`hard` = 否决（本站旧行为）。
///
/// 为什么默认 off：本轮拉源码核实过，UNIT3D 的 `config/announce.php`
/// 里 `connectable_check` **默认 false**，且 `peers.connectable` 的唯一消费者是
/// BON 积分条件；NexusPHP 建行时硬编码 `connectable='yes'`、按可达过滤的
/// 那句 peerlist SQL 是注释掉的；Gazelle 的 `xbt_files_users.connectable`
/// 默认 1 而 Ocelot 从不写它——**三家都只把可达性当信息位，没有一家拿它
/// 否决在种或计费**。我们的 BT 握手 + piece 抽查让 `conn=0` 比以前有意义，
/// 但拿它一票否决会把大量 NAT 后（没有映射入站端口）的真做种者判成不在种，
/// 且在种数/保种考核/濒危种救援一起塌。要强口径的站点显式开 hard。
///
/// 曾经写过 soft 档（"只留痕不否决"）后删掉：留痕本来就是
/// `snatches.connectable` 这一列，与 off 的行为没有任何差别——
/// 一档没有后果的枚举就是本轮一直在报的那种假开关。
pub(crate) fn conn_gate_hard() -> bool {
    gate_cell().load(std::sync::atomic::Ordering::Relaxed)
}

/// 每轮消费开头调一次（60s 节流；查询失败保留旧值，与 tracker 侧
/// guard_refresh 同一纪律——闸门档位不该因一次抖动被重置）。
pub(crate) async fn refresh_conn_gate(db: &sqlx::PgPool) {
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    static NEXT: std::sync::OnceLock<std::sync::Mutex<Instant>> =
        std::sync::OnceLock::new();
    let cell = NEXT.get_or_init(|| {
        std::sync::Mutex::new(
            Instant::now() - std::time::Duration::from_secs(60),
        )
    });
    {
        let Ok(g) = cell.lock() else { return };
        if g.elapsed() < std::time::Duration::from_secs(60) {
            return;
        }
    }
    let v: Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'connectable_gate'",
    )
    .fetch_optional(db)
    .await;
    let hard = match v {
        Ok(Some(t)) => t.trim() == "hard",
        Ok(None) => false,
        Err(e) => {
            tracing::warn!(%e, "connectable_gate 读取失败，保留上次档位");
            return;
        }
    };
    gate_cell().store(hard, Ordering::Relaxed);
    if let Ok(mut g) = cell.lock() {
        *g = Instant::now();
    }
}

fn gate_cell() -> &'static std::sync::atomic::AtomicBool {
    static G: std::sync::OnceLock<std::sync::atomic::AtomicBool> =
        std::sync::OnceLock::new();
    G.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
}

#[cfg(test)]
mod tests;

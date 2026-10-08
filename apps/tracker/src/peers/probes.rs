//! connectable 回连抽样候选选取（2026-10-07 保种组审计：自 table.rs 抽出，
//! 独立文件而非 impl 块——table.rs 已在 300 行门禁基线上，抽样策略单侧演化）。

use super::model::{PeerKey, CONN_UNTESTED};

/// 探测轮转周期（秒）：probe_loop 的 tick 与本文件两处轮转 epoch
/// 共用同一常量——改周期必须三处同源，否则轮转起点跳变。
pub(crate) const PROBE_PERIOD_SECS: usize = 300;

/// 一个 swarm 的抽样候选快照（table 侧收集，避免跨模块暴露内部表结构）。
/// 字段：(peer_key, ip, port, connectable, is_seeder)
pub(crate) struct SwarmCandidates {
    pub(crate) peers: Vec<(PeerKey, String, u16, i8, bool)>,
    /// 热 swarm = 存在 leecher（幽灵做种在此才有「伪装可上传」的收益）
    pub(crate) hot: bool,
}

/// connectable 抽样候选：优先未测（-1），其次轮替已测 peer（连通性会变化，
/// 需周期复测）。返回 (key, ip, port) 供 main 的 tokio 任务做 TCP 回连。
/// P2（2026-10-06 安全审计「幽灵做种」）：未测候选超编时按时间轮转起点
/// 截断——旧版固定取哈希序前 n 个，大池下排名靠后的未测 peer 可能永远
/// 轮不到（CONN_UNTESTED 长期滞留）。轮转后每个未测 peer 在
/// ceil(total/n) 轮内必被抽中一次。
/// 热度加权（2026-10-07 保种组审计 P2）：有 leecher 的 swarm 配额 n，
/// 冷 swarm 配额 n/4——幽灵做种只在「伪装可上传」有收益的场景（有人
/// 下载）才值得执法，探测预算优先花在热 swarm；冷 swarm 延后一轮复测
/// 不改变判定（connectable 本就是周期复测的滑动值）。
pub(crate) fn sample_probes(
    swarms: &[SwarmCandidates],
    n: usize,
) -> Vec<(PeerKey, String, u16)> {
    // 回连目标过滤（审计 10-07 P1-3 的纵深防御）：本环/未指定/组播/链路本地/
    // 非法串一律不回连。tracker 是拿**客户端上报**的地址发起出站连接的，
    // 不兜这层就成了内网端口探测器（实测过 ?ip=172.20.0.5&port=5432 连通）。
    let probeable = |ip: &str| crate::http_track::ip_trust::probeable_ip(ip);
    let mut out: Vec<(PeerKey, String, u16)> = Vec::with_capacity(n);
    let mut retriable: Vec<(PeerKey, String, u16)> = Vec::new();
    for s in swarms {
        let cap = if s.hot { n } else { n / 4 };
        for (key, ip, port, connectable, _seeder) in &s.peers {
            if !probeable(ip) {
                continue;
            }
            let bucket = if *connectable == CONN_UNTESTED {
                &mut out
            } else {
                &mut retriable
            };
            if bucket.len() < cap {
                bucket.push((key.clone(), ip.clone(), *port));
            }
        }
    }
    if out.len() > n {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as usize)
            .unwrap_or(0)
            / PROBE_PERIOD_SECS; // 同轮内稳定、跨轮前进
        let skip = epoch % out.len();
        out.rotate_left(skip);
        out.truncate(n);
    }
    // 二轮审计：retriable 也要轮转——旧版只取迭代序前 n 个已测 peer，
    // 大站已测池 >n 时其余永不复测，connectable 冻结（先正常做种测得
    // 可达、再撤监听伪造 announce，ghost_seed 永不触发）。
    if !retriable.is_empty() {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as usize)
            .unwrap_or(0)
            / PROBE_PERIOD_SECS;
        let skip = epoch % retriable.len();
        retriable.rotate_left(skip);
    }
    for r in retriable {
        if out.len() >= n {
            break;
        }
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(ip: &str, connectable: i8) -> SwarmCandidates {
        SwarmCandidates {
            hot: false,
            peers: vec![(
                PeerKey {
                    info_hash: "aa".repeat(20),
                    peer_id: "bb".repeat(20),
                },
                ip.to_string(),
                51413,
                connectable,
                true,
            )],
        }
    }

    #[test]
    fn never_probes_reserved_or_unparsable_addresses() {
        for bad in [
            "127.0.0.1",
            "::1",
            "224.0.0.5",
            "169.254.1.1",
            "fe80::1",
            "0.0.0.0",
            "not-an-ip",
        ] {
            let got = sample_probes(&[c(bad, CONN_UNTESTED)], 10);
            assert!(got.is_empty(), "{bad} 不该被回连");
        }
        // 真实场景里 socket 对端就是内网地址（单机 docker 栈 172.x）——必须照测
        assert_eq!(
            sample_probes(&[c("172.20.0.1", CONN_UNTESTED)], 10).len(),
            1
        );
        assert_eq!(sample_probes(&[c("8.8.8.8", CONN_UNTESTED)], 10).len(), 1);
    }
}

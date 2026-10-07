//! connectable 回连抽样候选选取（2026-10-07 保种组审计：自 table.rs 抽出，
//! 独立文件而非 impl 块——table.rs 已在 300 行门禁基线上，抽样策略单侧演化）。

use super::model::{PeerKey, CONN_UNTESTED};

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
    let mut out: Vec<(PeerKey, String, u16)> = Vec::with_capacity(n);
    let mut retriable: Vec<(PeerKey, String, u16)> = Vec::new();
    for s in swarms {
        let cap = if s.hot { n } else { n / 4 };
        for (key, ip, port, connectable, _seeder) in &s.peers {
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
            / 300; // 抽样周期 5min，同轮内稳定、跨轮前进
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
            / 300;
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

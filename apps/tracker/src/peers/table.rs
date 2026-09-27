//! 内存 Peer 表（§5.2 tracker 职责：按 info_hash 分桶 + 桶内 HashMap，90s 超时淘汰）。
//!
//! P0-3（0071）：此前是单张 `DashMap<PeerKey, Peer>`，snapshot/counts 对**全体在线 peer**
//! 全表遍历再按 info_hash 过滤——热门 swarm 一次 announce 就扫全表。现改为
//! `DashMap<info_hash, Swarm>` 二级索引：snapshot/counts 只碰对应桶，复杂度从
//! O(全体 peer) 降到 O(该 swarm)；DashMap 分片锁竞争同时缓解（每桶独立 entry）。

use dashmap::DashMap;
use std::collections::HashMap;
use std::time::Duration;

use super::model::{
    CompactPeer, CompactPeer6, Peer, PeerKey, Snapshot, CONN_DEAD, CONN_OK,
    CONN_UNTESTED,
};

pub(crate) const PEER_TIMEOUT: Duration = Duration::from_secs(90);
/// 0077 分档 TTL（U3D ACTIVE_PEER_TTL 口径）：做种中的 peer 放宽——
/// interval 1800s 下 90s 一刀切会让挂种大户每 90s 全量重建内存表；
/// 3720s = 2×interval+120 冗余，断线种子在两个周期内自然除名。
pub(crate) const SEEDER_TIMEOUT: Duration = Duration::from_secs(3720);
const MAX_PEERS_RESPONSE: usize = 50;

/// 单个 swarm 的桶：peer_id hex → Peer
#[derive(Default)]
struct Swarm {
    peers: HashMap<String, Peer>,
}

#[derive(Default)]
pub struct PeerTable {
    swarms: DashMap<String, Swarm>,
}

impl PeerTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// 全量导出（Redis 快照用）：(info_hash, peers) 列表。
    /// 大表下 JSON 体积 ≈ 每人 ~200B；10 万 peer ≈ 20MB，60s 周期可接受。
    pub fn export(&self) -> Vec<(String, Vec<Peer>)> {
        self.swarms
            .iter()
            .map(|s| (s.key().clone(), s.peers.values().cloned().collect()))
            .collect()
    }

    /// 快照恢复（启动预热）：客户端 30min 内重 announce 可自愈，预热只为
    /// 缩短空窗期——只恢复未超时的 peer（按 last_seen + 分档 TTL 判定）。
    pub fn restore(&self, snap: Vec<(String, Vec<Peer>)>) -> usize {
        let now = chrono::Utc::now();
        let mut n = 0;
        for (_ih, peers) in snap {
            for p in peers {
                let ttl = if p.left == 0 {
                    SEEDER_TIMEOUT
                } else {
                    PEER_TIMEOUT
                };
                if now
                    .signed_duration_since(p.last_seen)
                    .to_std()
                    .unwrap_or_default()
                    < ttl
                {
                    self.upsert(p);
                    n += 1;
                }
            }
        }
        n
    }

    /// 活跃 peer 总数（/metrics 用；近似值——不触发 GC，精确性由各桶惰性 GC 保证）
    pub fn len(&self) -> usize {
        self.swarms.iter().map(|s| s.peers.len()).sum()
    }

    /// 在线 swarm 数（/metrics 用）
    pub fn swarms(&self) -> usize {
        self.swarms.len()
    }

    pub fn upsert(&self, mut peer: Peer) {
        let mut s = self.swarms.entry(peer.key.info_hash.clone()).or_default();
        // 重复 announce：保留此前的回连测量结果（抽样周期 5min，不能被每次 announce 重置）
        if let Some(old) = s.peers.get(&peer.key.peer_id) {
            peer.connectable = old.connectable;
        }
        s.peers.insert(peer.key.peer_id.clone(), peer);
    }

    /// 查询 peer 当前回连状态（-1 未测）
    pub fn connectable_of(&self, key: &PeerKey) -> i8 {
        self.swarms
            .get(&key.info_hash)
            .map(|s| {
                s.peers
                    .get(&key.peer_id)
                    .map_or(CONN_UNTESTED, |p| p.connectable)
            })
            .unwrap_or(CONN_UNTESTED)
    }

    pub fn remove(&self, key: &PeerKey) {
        if let Some(mut s) = self.swarms.get_mut(&key.info_hash) {
            s.peers.remove(&key.peer_id);
            if s.peers.is_empty() {
                drop(s);
                self.swarms
                    .remove_if(&key.info_hash, |_, sw| sw.peers.is_empty());
            }
        }
    }

    /// 取同一 info_hash 的活跃 peer（排除自己，numwant 上限），v4/v6 分列。
    /// BEP-7：v6 peer 不再被丢弃；只扫本 swarm 桶（P0-3）。
    pub fn snapshot(
        &self,
        info_hash: &str,
        numwant: usize,
        exclude: &str,
    ) -> Snapshot {
        self.gc_swarm(info_hash);
        let limit = numwant.clamp(1, MAX_PEERS_RESPONSE);
        let mut snap = Snapshot::default();
        let Some(s) = self.swarms.get(info_hash) else {
            return snap;
        };
        for p in s.peers.values() {
            if snap.v4.len() + snap.v6.len() >= limit {
                break;
            }
            if p.key.peer_id == exclude {
                continue;
            }
            let port = p.port;
            if let Ok(v6) = p.ip.parse::<std::net::Ipv6Addr>() {
                snap.v6.push(CompactPeer6 {
                    ip: v6.octets(),
                    port,
                });
            } else if let Ok(v4) = p.ip.parse::<std::net::Ipv4Addr>() {
                snap.v4.push(CompactPeer {
                    ip: v4.octets(),
                    port,
                });
            }
            // 其余非法字符串（历史脏数据）丢弃
        }
        snap
    }

    pub fn count_seeders(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        self.swarms
            .get(info_hash)
            .map_or(0, |s| s.peers.values().filter(|p| p.is_seeder()).count())
    }

    pub fn count_leechers(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        self.swarms
            .get(info_hash)
            .map_or(0, |s| s.peers.values().filter(|p| !p.is_seeder()).count())
    }

    pub fn counts(&self, info_hash: &str) -> (usize, usize) {
        (
            self.count_seeders(info_hash),
            self.count_leechers(info_hash),
        )
    }

    /// connectable 抽样候选：优先未测（-1），其次轮替已测 peer（连通性会变化，需周期复测）。
    /// 返回 (key, ip, port) 供 main 的 tokio 任务做 TCP 回连。
    pub fn sample_probes(&self, n: usize) -> Vec<(PeerKey, String, u16)> {
        let mut out = Vec::with_capacity(n);
        let mut retriable: Vec<(PeerKey, String, u16)> = Vec::new();
        for s in self.swarms.iter() {
            for p in s.peers.values() {
                if out.len() >= n {
                    break;
                }
                match p.connectable {
                    CONN_UNTESTED => {
                        out.push((p.key.clone(), p.ip.clone(), p.port))
                    }
                    _ => {
                        if retriable.len() < n {
                            retriable.push((
                                p.key.clone(),
                                p.ip.clone(),
                                p.port,
                            ))
                        }
                    }
                }
            }
            if out.len() >= n {
                break;
            }
        }
        for r in retriable {
            if out.len() >= n {
                break;
            }
            out.push(r);
        }
        out
    }

    /// 回连结果写回（peer 可能在检测间隙超时下线——不存在则忽略）
    pub fn set_connectable(&self, key: &PeerKey, reachable: bool) {
        if let Some(mut s) = self.swarms.get_mut(&key.info_hash) {
            if let Some(p) = s.peers.get_mut(&key.peer_id) {
                p.connectable = if reachable { CONN_OK } else { CONN_DEAD };
            }
        }
    }

    /// 某账号全部存活 peer 是否回连可达（供管理侧展示；true=无任何可达 peer）
    #[allow(dead_code)] // 预留管理端「回连全不可达名单」查询；测试已覆盖语义
    pub fn all_unreachable(&self, user_id: i64) -> bool {
        for s in self.swarms.iter() {
            for p in s.peers.values() {
                if p.user_id == user_id && p.connectable == CONN_OK {
                    return false;
                }
            }
        }
        true
    }

    /// 全表 GC（/metrics 抓取周期触发，平时各桶在 snapshot/counts 时惰性清理）
    pub fn gc_all(&self) {
        let now = chrono::Utc::now();
        self.swarms.retain(|_, s| {
            s.peers.retain(|_, p| alive(p, &now));
            !s.peers.is_empty()
        });
    }

    /// 单桶惰性 GC（snapshot/counts 热路径触发）
    fn gc_swarm(&self, info_hash: &str) {
        let now = chrono::Utc::now();
        if let Some(mut s) = self.swarms.get_mut(info_hash) {
            s.peers.retain(|_, p| alive(p, &now));
        }
    }
}

/// 0077 分档存活判定：做种 peer 用 SEEDER_TIMEOUT，其余 PEER_TIMEOUT。
fn alive(p: &Peer, now: &chrono::DateTime<chrono::Utc>) -> bool {
    let timeout = if p.left == 0 {
        SEEDER_TIMEOUT
    } else {
        PEER_TIMEOUT
    };
    now.signed_duration_since(p.last_seen)
        .to_std()
        .unwrap_or_default()
        < timeout
}

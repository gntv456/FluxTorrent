//! 内存 Peer 表（§5.2 tracker 职责：按 info_hash 分桶 + 桶内 HashMap，
//! 分档 TTL 淘汰，TTL 随 announce_interval 伸缩——见 peers::ttl_for）。
//!
//! P0-3（0071）：snapshot/counts 曾对全体在线 peer 全表遍历再过滤——热门 swarm
//! 一次 announce 就扫全表。现为 `DashMap<info_hash, Swarm>` 二级索引：只碰对应桶。

use dashmap::DashMap;
use std::collections::HashMap;

use super::model::{
    CompactPeer, CompactPeer6, Peer, PeerKey, Snapshot, CONN_DEAD, CONN_OK,
    CONN_UNTESTED,
};
use super::ttl_for;

const MAX_PEERS_RESPONSE: usize = 50;
/// 同账号同 swarm 的 peer 上限（审计 10-06）：防单账号刷随机 peer_id 制造
/// 影子 peer 抬高在线数；超限淘汰该账号最旧的。
const MAX_PEERS_PER_USER: usize = 10;

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
                if now
                    .signed_duration_since(p.last_seen)
                    .to_std()
                    .unwrap_or_default()
                    < ttl_for(p.left)
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
        let is_new = !s.peers.contains_key(&peer.key.peer_id);
        let uid = peer.user_id;
        s.peers.insert(peer.key.peer_id.clone(), peer);
        // 每账号每 swarm 配额：新 peer 且桶已超阈值才扫描（小 swarm 零开销）
        if is_new && s.peers.len() > MAX_PEERS_PER_USER {
            let mut mine: Vec<_> = s
                .peers
                .values()
                .filter(|p| p.user_id == uid)
                .map(|p| (p.last_seen, p.key.peer_id.clone()))
                .collect();
            if mine.len() > MAX_PEERS_PER_USER {
                let over = mine.len() - MAX_PEERS_PER_USER;
                mine.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                for (_, pid) in mine.into_iter().take(over) {
                    s.peers.remove(&pid);
                }
            }
        }
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

    /// stopped 归属校验版移除（审计 10-06 第 2 条）：peer_id 客户端自报，旧 remove
    /// 使任意账号可用他人 peer_id 把对方踢下线。仅当现存 peer 属同一 user 才
    /// 移除；返回是否移除。
    pub fn remove_owned(&self, key: &PeerKey, user_id: i64) -> bool {
        let Some(mut s) = self.swarms.get_mut(&key.info_hash) else {
            return false;
        };
        let owned = s
            .peers
            .get(&key.peer_id)
            .is_some_and(|p| p.user_id == user_id);
        if owned {
            s.peers.remove(&key.peer_id);
        }
        let empty = s.peers.is_empty();
        drop(s); // 必须先放掉写锁，否则同分片 remove_if 死锁
        if empty {
            self.swarms
                .remove_if(&key.info_hash, |_, sw| sw.peers.is_empty());
        }
        owned
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
        // ZT81：允许 numwant=0（客户端明确不要 peer 列表时不返回）；上限照旧钳死
        let limit = numwant.min(MAX_PEERS_RESPONSE);
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
            // ZT81：port=0 的 peer 不可连接，不下发给其他客户端
            if p.port == 0 {
                continue;
            }
            let port = p.port;
            let peer_id = crate::peers::peer_id_bytes(&p.key.peer_id);
            if let Ok(v6) = p.ip.parse::<std::net::Ipv6Addr>() {
                snap.v6.push(CompactPeer6 {
                    ip: v6.octets(),
                    port,
                });
            } else if let Ok(v4) = p.ip.parse::<std::net::Ipv4Addr>() {
                snap.v4.push(CompactPeer {
                    ip: v4.octets(),
                    port,
                    peer_id,
                });
            }
            // 其余非法字符串（历史脏数据）丢弃
        }
        snap
    }

    // 计数与 snapshot 同口径（审计 10-06）：port=0 不可连接，旧实现计数仍含 → 虚高。
    pub fn count_seeders(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        self.swarms.get(info_hash).map_or(0, |s| {
            s.peers
                .values()
                .filter(|p| p.is_seeder() && p.port != 0)
                .count()
        })
    }

    pub fn count_leechers(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        self.swarms.get(info_hash).map_or(0, |s| {
            s.peers
                .values()
                .filter(|p| !p.is_seeder() && p.port != 0)
                .count()
        })
    }

    pub fn counts(&self, info_hash: &str) -> (usize, usize) {
        (
            self.count_seeders(info_hash),
            self.count_leechers(info_hash),
        )
    }

    /// connectable 抽样候选：优先未测（-1），其次轮替已测 peer（连通性会变化，需周期复测）。
    /// 返回 (key, ip, port) 供 main 的 tokio 任务做 TCP 回连。
    /// P2（2026-10-06 安全审计「幽灵做种」）：未测候选超编时按时间轮转起点
    /// 截断——旧版固定取哈希序前 n 个，大池下排名靠后的未测 peer 可能永远
    /// 轮不到（CONN_UNTESTED 长期滞留），而「不可达且零上传」过滤对未测
    /// peer 不生效。轮转后每个未测 peer 在 ceil(total/n) 轮内必被抽中一次。
    pub fn sample_probes(&self, n: usize) -> Vec<(PeerKey, String, u16)> {
        let mut out: Vec<(PeerKey, String, u16)> = Vec::with_capacity(n);
        let mut retriable: Vec<(PeerKey, String, u16)> = Vec::new();
        for s in self.swarms.iter() {
            for p in s.peers.values() {
                if p.connectable == CONN_UNTESTED {
                    out.push((p.key.clone(), p.ip.clone(), p.port));
                } else if retriable.len() < n {
                    retriable.push((p.key.clone(), p.ip.clone(), p.port));
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

    /// 单桶惰性 GC（snapshot/counts 热路径触发）。
    /// ZT81（2026-10-02）：清理后若桶已空则**移除整桶**——原实现只 retain peers、
    /// 不删空桶，而空桶仅由 `remove()`（event=stopped）清理，于是「只被 announce
    /// 一次、之后再无人问津」的 info_hash 会永久驻留。实测 3,000 个幽灵 swarm 让
    /// 常驻内存 +1.77 MiB（≈590 B/个）；1 GiB 上限下约 170 万个即 OOM，而单账号
    /// 限速 1800/min，约 16 小时就能造出来（tracker 是单进程 SPOF）。
    fn gc_swarm(&self, info_hash: &str) {
        let now = chrono::Utc::now();
        if let Some(mut s) = self.swarms.get_mut(info_hash) {
            s.peers.retain(|_, p| alive(p, &now));
            let empty = s.peers.is_empty();
            drop(s); // 必须先放掉写锁，否则同分片 remove_if 死锁
            if empty {
                self.swarms
                    .remove_if(info_hash, |_, sw| sw.peers.is_empty());
            }
        }
    }
}

/// 分档存活判定：做种 2×interval+120，其余 interval+120（见 peers::ttl_for）。
fn alive(p: &Peer, now: &chrono::DateTime<chrono::Utc>) -> bool {
    now.signed_duration_since(p.last_seen)
        .to_std()
        .unwrap_or_default()
        < ttl_for(p.left)
}

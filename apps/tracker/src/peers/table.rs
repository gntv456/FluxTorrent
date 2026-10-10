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

mod table_persist;
mod table_query;

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

    /// 活跃 peer 总数（/metrics 用；近似值——不触发 GC，精确性由各桶惰性 GC 保证）
    pub fn len(&self) -> usize {
        self.swarms.iter().map(|s| s.peers.len()).sum()
    }

    /// 在线 swarm 数（/metrics 用）
    pub fn swarms(&self) -> usize {
        self.swarms.len()
    }

    /// 写入/刷新一条 peer。返回 false = 槽位已被**其他账号**占用（顶号企图）：
    /// 既不改写对方 ip/port/left，也不把对方的可达测量传给顶号者。
    /// 审计 10-07 P1-1 实测：旧版无归属校验 ⇒ 先顶号再 stopped 就能把从未停种的
    /// 受害者抹出 swarm，或搬运「回连可达」信用绕过幽灵做种的 conn 判据。
    pub fn upsert(&self, mut peer: Peer) -> bool {
        let mut s = self.swarms.entry(peer.key.info_hash.clone()).or_default();
        // 重复 announce：保留此前的回连测量结果（抽样周期 5min，不能被每次 announce 重置）
        if let Some(old) = s.peers.get(&peer.key.peer_id) {
            if old.user_id != peer.user_id {
                return false;
            }
            peer.connectable = old.connectable;
            // 另一族的端点要留下：同一 peer_id 的 v4 周期 announce 不该抹掉
            // 它先前报过的 v6（反之亦然）——两族是同一个客户端的两条通路。
            if peer.v4.is_none() {
                peer.v4 = old.v4.clone();
            }
            if peer.v6.is_none() {
                peer.v6 = old.v6.clone();
            }
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
        true
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
    /// 回连结果写回（peer 可能在检测间隙超时下线——不存在则忽略）
    pub fn set_connectable(&self, key: &PeerKey, reachable: bool) {
        self.set_conn_state(key, if reachable { CONN_OK } else { CONN_DEAD });
    }

    /// 写回四档探测结论（2026-10-08）：见 `model::CONN_SUSPECT` 的定义。
    /// 只写确定的结论；`CONN_UNTESTED` 不应由此路径写入（未探测就保持
    /// 旧值，避免把「没轮到」误当成「测过了」）。
    pub fn set_conn_state(&self, key: &PeerKey, state: i8) {
        if state == CONN_UNTESTED {
            return;
        }
        if let Some(mut s) = self.swarms.get_mut(&key.info_hash) {
            if let Some(p) = s.peers.get_mut(&key.peer_id) {
                p.connectable = state;
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

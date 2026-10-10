//! Peer 表的读取侧（snapshot / counts / 探测候选 / 佐证目标解析）。
//!
//! 从 table.rs 拆出（300 行门禁）。作为 `peers::table` 的子模块，可直接读
//! 父模块的私有 `swarms` 与 `gc_swarm`，无需为此放宽可见性。

use super::PeerTable;
use super::{CompactPeer, CompactPeer6, Snapshot, MAX_PEERS_RESPONSE};

impl PeerTable {
    /// 取同一 info_hash 的活跃 peer（排除自己，numwant 上限），v4/v6 分列。
    /// BEP-7：v6 peer 不再被丢弃；只扫本 swarm 桶（P0-3）。
    /// 取一份可下发的 peer 列表。`self_user` = 请求者账号：
    /// 同账号的 peer **一条都不下发**（UNIT3D/Ocelot 口径——自己的 peer 自己
    /// 早就知道，发回去既无信息量，又把「同一账号有几个 peer_id、各自
    /// ip:port」这类可被用来对撞顶号/影子 peer 的内部结构暴露给客户端）。
    /// 传 0 表示不排除（真实 user_id 恒 >= 1，单测里用它保留「全量可见」语义）。
    pub fn snapshot(
        &self,
        info_hash: &str,
        numwant: usize,
        self_user: i64,
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
            if p.user_id == self_user {
                continue;
            }
            // ZT81：port=0 的 peer 不可连接，不下发给其他客户端。
            // 两族各自下发（BEP 7）：v6 对端不再因为「这台客户端最后一次
            // announce 是 v4」而被塞一个不可达地址。
            let peer_id = crate::peers::peer_id_bytes(&p.key.peer_id);
            for ep in p.endpoints() {
                if snap.v4.len() + snap.v6.len() >= limit {
                    break;
                }
                // 其余非法字符串（历史脏数据）丢弃
                if ep.port == 0 {
                    continue;
                }
                if let Ok(v6) = ep.ip.parse::<std::net::Ipv6Addr>() {
                    snap.v6.push(CompactPeer6 {
                        ip: v6.octets(),
                        port: ep.port,
                    });
                } else if let Ok(v4) = ep.ip.parse::<std::net::Ipv4Addr>() {
                    snap.v4.push(CompactPeer {
                        ip: v4.octets(),
                        port: ep.port,
                        peer_id,
                    });
                }
            }
        }
        snap
    }

    // 计数与 snapshot 同口径（审计 10-06）：port=0 不可连接，旧实现计数仍含 → 虚高。
    //
    // P2-6（2026-10-07）：再按 user_id 去重——peer_id 客户端自报，同一账号
    // 可用多个随机 peer_id 在同一 swarm 里注册多条记录，把实时 seeders
    // 灌成自己的倍数（每账号已有 MAX_PEERS_PER_USER=10 的表配额，但 10 条
    // 仍足以让「1 个做种的人」显示成 10 个）。权威计数在 snatches（按
    // user+torrent 唯一），实时口径与之对齐：同一用户在同一 swarm 内
    // seeder / leecher 各只计一次。
    pub fn count_seeders(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        let mut seen = std::collections::HashSet::new();
        self.swarms.get(info_hash).map_or(0, |s| {
            s.peers
                .values()
                .filter(|p| p.is_seeder() && p.has_listening())
                .filter(|p| seen.insert(p.user_id))
                .count()
        })
    }

    pub fn count_leechers(&self, info_hash: &str) -> usize {
        self.gc_swarm(info_hash);
        let mut seen = std::collections::HashSet::new();
        self.swarms.get(info_hash).map_or(0, |s| {
            s.peers
                .values()
                .filter(|p| !p.is_seeder() && p.has_listening())
                .filter(|p| seen.insert(p.user_id))
                .count()
        })
    }

    pub fn counts(&self, info_hash: &str) -> (usize, usize) {
        (
            self.count_seeders(info_hash),
            self.count_leechers(info_hash),
        )
    }

    /// 回连抽样候选快照（策略在 probes.rs：热 swarm 优先 + 未测轮转；
    /// 2026-10-07 保种组审计 P2 抽出，table.rs 只负责收集不负责选取）。
    pub(crate) fn probe_candidates(
        &self,
    ) -> Vec<crate::peers::probes::SwarmCandidates> {
        self.swarms
            .iter()
            .map(|s| crate::peers::probes::SwarmCandidates {
                hot: s.peers.values().any(|p| !p.is_seeder()),
                peers: s
                    .peers
                    .values()
                    .filter_map(|p| {
                        // 回连探测按主端点（v4 优先）单值口径：`connectable`
                        // 本来就是每 peer 一个值，逐族探测会让测量值在两族间
                        // 来回刷新（10-08 的粘滞判据吃的就是这个单值）。
                        let ep = p.primary()?;
                        Some((
                            p.key.clone(),
                            ep.ip.clone(),
                            ep.port,
                            p.connectable,
                            p.is_seeder(),
                        ))
                    })
                    .collect(),
            })
            .collect()
    }

    /// 交叉上报校验：给定 info_hash + peer_id，返回该 peer 的 user_id，
    /// 仅当它是**当前存活且在做种**（left=0）的真实 peer 时返回。
    /// leecher 声明「我从这个 peer 下载了 N 字节」时，tracker 用它把
    /// peer_id 解析成可计费的上传者账号，并拒绝：
    ///   · 不存在的 peer_id（凭空捏造上传者）
    ///   · 指向自己（自己报自己下载 = 自己给自己刷上传）
    ///   · 指向 leecher/非做种 peer（只有做种者才能提供上传）
    /// 存活判定顺带做了 GC 语义外的惰性检查：peer 表本身按 TTL 除名，
    /// 这里只认表内现有行。
    pub fn corroboration_target(
        &self,
        info_hash: &str,
        peer_id: &str,
        leecher_id: i64,
    ) -> Option<i64> {
        self.gc_swarm(info_hash);
        let s = self.swarms.get(info_hash)?;
        let p = s.peers.get(peer_id)?;
        if !p.is_seeder() || !p.has_listening() {
            return None;
        }
        if p.user_id == leecher_id {
            return None; // 自报自下载
        }
        Some(p.user_id)
    }
}

//! Peer 表的快照导出/恢复（从 table.rs 拆出，同受 300 行门禁约束）。
//!
//! 只在降级/预热路径用，不在 announce 热路径上。作为 `peers::table` 的
//! 子模块，可直接读写父模块的私有 `swarms` 字段，无需为此放宽可见性。

use super::PeerTable;
use crate::peers::{ttl_for, Peer};

impl PeerTable {
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
}

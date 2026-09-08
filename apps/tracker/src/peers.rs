//! 内存 Peer 表（§5.2 tracker 职责：DashMap 分片锁，60s 超时淘汰）。

use dashmap::DashMap;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerKey {
    pub info_hash: String,
    pub peer_id: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Peer {
    pub key: PeerKey,
    pub ip: String,
    pub port: u16,
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub user_id: i64,
}

impl Peer {
    pub fn is_seeder(&self) -> bool {
        self.left == 0
    }
}

#[derive(Clone, Copy)]
pub struct CompactPeer {
    pub ip: [u8; 4],
    pub port: u16,
}

#[derive(Default)]
pub struct PeerTable {
    inner: DashMap<PeerKey, Peer>,
}

const PEER_TIMEOUT: Duration = Duration::from_secs(90);

impl PeerTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, peer: Peer) {
        self.inner.insert(peer.key.clone(), peer);
    }

    pub fn remove(&self, key: &PeerKey) {
        self.inner.remove(key);
    }

    /// 取同一 info_hash 的活跃 peer（排除自己，numwant 上限）
    pub fn snapshot(&self, info_hash: &str, numwant: usize, exclude: &str) -> Vec<CompactPeer> {
        self.gc();
        let limit = numwant.min(MAX_PEERS_RESPONSE);
        self.inner
            .iter()
            .filter(|e| e.key().info_hash == info_hash && e.key().peer_id != exclude)
            .take(limit)
            .filter_map(|e| {
                let ip: Vec<u8> = e
                    .value()
                    .ip
                    .split('.')
                    .filter_map(|p| p.parse::<u8>().ok())
                    .collect();
                if ip.len() == 4 {
                    Some(CompactPeer {
                        ip: [ip[0], ip[1], ip[2], ip[3]],
                        port: e.value().port,
                    })
                } else {
                    None // IPv6 → 简化版暂不进 compact 列表
                }
            })
            .collect()
    }

    pub fn count_seeders(&self, info_hash: &str) -> usize {
        self.gc();
        self.inner
            .iter()
            .filter(|e| e.key().info_hash == info_hash && e.value().is_seeder())
            .count()
    }

    pub fn count_leechers(&self, info_hash: &str) -> usize {
        self.gc();
        self.inner
            .iter()
            .filter(|e| e.key().info_hash == info_hash && !e.value().is_seeder())
            .count()
    }

    pub fn counts(&self, info_hash: &str) -> (usize, usize) {
        (
            self.count_seeders(info_hash),
            self.count_leechers(info_hash),
        )
    }

    /// 超时淘汰（惰性触发）
    fn gc(&self) {
        let now = chrono::Utc::now();
        self.inner.retain(|_, p| {
            now.signed_duration_since(p.last_seen)
                .to_std()
                .unwrap_or_default()
                < PEER_TIMEOUT
        });
    }
}

const MAX_PEERS_RESPONSE: usize = 50;

/// BEP3 bencode announce 响应（compact 模式：6 字节/peer）
pub fn bencode_announce(
    complete: i64,
    incomplete: i64,
    downloaded: i64,
    peers: &[CompactPeer],
) -> String {
    let mut peers_bytes = Vec::with_capacity(peers.len() * 6);
    for p in peers {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    // 二进制 peers 无法安全放进 String —— 用 lossy 转换（HTTP body 由调用方以 bytes 发出）
    format!(
        "d8:completei{}e10:incompletei{}e10:downloadedi{}e5:peers{}:{}e",
        complete,
        incomplete,
        downloaded,
        peers_bytes.len(),
        String::from_utf8_lossy(&peers_bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_peer(ih: &str, pid: &str, left: i64) -> Peer {
        Peer {
            key: PeerKey {
                info_hash: ih.into(),
                peer_id: pid.into(),
            },
            ip: "10.0.0.1".into(),
            port: 51413,
            uploaded: 0,
            downloaded: 0,
            left,
            last_seen: chrono::Utc::now(),
            user_id: 1,
        }
    }

    #[test]
    fn seed_leech_count_and_exclusion() {
        let t = PeerTable::new();
        t.upsert(mk_peer("abc", "p1", 0)); // seeder
        t.upsert(mk_peer("abc", "p2", 100)); // leecher
        t.upsert(mk_peer("xyz", "p3", 0)); // 另一种子
        assert_eq!(t.counts("abc"), (1, 1));
        let snap = t.snapshot("abc", 50, "p1");
        assert_eq!(snap.len(), 1); // 排除自己
    }

    #[test]
    fn stopped_removes() {
        let t = PeerTable::new();
        let key = PeerKey {
            info_hash: "abc".into(),
            peer_id: "p1".into(),
        };
        t.upsert(mk_peer("abc", "p1", 0));
        t.remove(&key);
        assert_eq!(t.counts("abc"), (0, 0));
    }

    #[test]
    fn bencode_shape() {
        let body = bencode_announce(1, 2, 3, &[]);
        assert!(body.starts_with("d8:completei1e"));
        assert!(body.contains("5:peers0:"));
    }
}

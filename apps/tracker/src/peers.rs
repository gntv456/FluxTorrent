//! 内存 Peer 表（§5.2 tracker 职责：DashMap 分片锁，90s 超时淘汰）。

use dashmap::DashMap;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerKey {
    /// info_hash 的 hex 编码（40 字符），与 DB/事件流口径一致
    pub info_hash: String,
    /// peer_id 的 hex 编码
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
const MAX_PEERS_RESPONSE: usize = 50;

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
        let limit = numwant.clamp(1, MAX_PEERS_RESPONSE);
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

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// BEP3 bencode announce 响应（compact 模式：6 字节/peer）。
/// 返回原始字节 —— peer 列表是二进制，绝不可经 String/UTF-8 转换（会损坏数据）。
pub fn bencode_announce(
    complete: i64,
    incomplete: i64,
    downloaded: i64,
    peers: &[CompactPeer],
) -> Vec<u8> {
    let mut peers_bytes = Vec::with_capacity(peers.len() * 6);
    for p in peers {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    let mut out = format!(
        "d8:completei{complete}e10:incompletei{incomplete}e10:downloadedi{downloaded}e5:peers{}:",
        peers_bytes.len()
    )
    .into_bytes();
    out.extend_from_slice(&peers_bytes);
    out.extend_from_slice(b"e");
    out
}

/// BEP3 scrape 响应：files 字典键为 20 字节原始 info_hash。
pub fn bencode_scrape(files: &[(Vec<u8>, usize, usize)]) -> Vec<u8> {
    let mut out = b"d5:filesd".to_vec();
    for (ih, seeders, leechers) in files {
        out.extend_from_slice(b"20:");
        out.extend_from_slice(ih);
        out.extend_from_slice(
            format!("d8:completei{seeders}e10:incompletei{leechers}e10:downloadedi0ee").as_bytes(),
        );
    }
    out.extend_from_slice(b"e");
    out
}

/// 手工 percent-decode：BT 客户端的 info_hash/peer_id 是任意字节的 URL 编码（含 %00-%FF），
/// 标准库 Query 反序列化走 UTF-8 会失败或损坏 —— 必须在字节层解码。
pub fn percent_decode(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            let hi = (raw[i + 1] as char).to_digit(16);
            let lo = (raw[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    out
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
    fn bencode_announce_binary_integrity() {
        let body = bencode_announce(
            1,
            2,
            3,
            &[CompactPeer {
                ip: [10, 0, 0, 1],
                port: 0xC201, // 高字节非 ASCII —— 验证不经 UTF-8 损坏
            }],
        );
        assert!(body.windows(6).any(|w| w == [10, 0, 0, 1, 0xC2, 0x01]));
        assert!(body.starts_with(b"d8:completei1e"));
        assert!(body.ends_with(b"e"));
    }

    #[test]
    fn percent_decode_binary_safe() {
        assert_eq!(percent_decode(b"%98%72%0b"), vec![0x98, 0x72, 0x0b]);
        assert_eq!(percent_decode(b"a%20b"), b"a b".to_vec());
        assert_eq!(percent_decode(b"plain"), b"plain".to_vec());
        assert_eq!(percent_decode(b"100%"), b"100%".to_vec()); // 尾部孤立 % 保留
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex(&[0x98, 0x72, 0x0b]), "98720b");
    }
}

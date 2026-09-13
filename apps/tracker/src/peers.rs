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

/// BEP-7：IPv6 peer（16 字节 IP + 2 字节端口，进响应的 peers6 字段）。
/// 教育网（CERNET2）IPv6 覆盖率极高，纯 v6 用户拿不到 v4 peer —— 不实现 peers6 等于拒服务。
#[derive(Clone, Copy)]
pub struct CompactPeer6 {
    pub ip: [u8; 16],
    pub port: u16,
}

/// 同一 info_hash 的 peer 快照（v4/v6 分列，响应里分别进 peers / peers6）
#[derive(Default)]
pub struct Snapshot {
    pub v4: Vec<CompactPeer>,
    pub v6: Vec<CompactPeer6>,
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

    /// 活跃 peer 总数（/metrics 用）
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn upsert(&self, peer: Peer) {
        self.inner.insert(peer.key.clone(), peer);
    }

    pub fn remove(&self, key: &PeerKey) {
        self.inner.remove(key);
    }

    /// 取同一 info_hash 的活跃 peer（排除自己，numwant 上限），v4/v6 分列。
    /// BEP-7：v6 peer 不再被丢弃（此前简化版只回 v4，纯 v6 用户拿不到任何 peer）。
    pub fn snapshot(&self, info_hash: &str, numwant: usize, exclude: &str) -> Snapshot {
        self.gc();
        let limit = numwant.clamp(1, MAX_PEERS_RESPONSE);
        let mut snap = Snapshot::default();
        for e in self
            .inner
            .iter()
            .filter(|e| e.key().info_hash == info_hash && e.key().peer_id != exclude)
            .take(limit)
        {
            let port = e.value().port;
            let ip = &e.value().ip;
            if let Ok(v6) = ip.parse::<std::net::Ipv6Addr>() {
                snap.v6.push(CompactPeer6 {
                    ip: v6.octets(),
                    port,
                });
            } else if let Ok(v4) = ip.parse::<std::net::Ipv4Addr>() {
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

/// BEP3 bencode announce 响应（compact 模式：v4 6 字节/peer，BEP-7 v6 18 字节/peer 进 peers6）。
/// interval / min interval 为 BEP3 强制字段：告知客户端汇报间隔，
/// 缺失时部分客户端会按自身默认值频繁重发 —— 这是高频 announce 的诱因之一。
/// 返回原始字节 —— peer 列表是二进制，绝不可经 String/UTF-8 转换（会损坏数据）。
pub fn bencode_announce(
    complete: i64,
    incomplete: i64,
    downloaded: i64,
    peers: &[CompactPeer],
    peers6: &[CompactPeer6],
    interval: i64,
    min_interval: i64,
) -> Vec<u8> {
    let mut peers_bytes = Vec::with_capacity(peers.len() * 6);
    for p in peers {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    let mut peers6_bytes = Vec::with_capacity(peers6.len() * 18);
    for p in peers6 {
        peers6_bytes.extend_from_slice(&p.ip);
        peers6_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    // 字典键序按字节序（BEP3）：… peers < peers6（前缀短者在前）
    let mut out = format!(
        "d8:completei{complete}e10:incompletei{incomplete}e10:downloadedi{downloaded}e\
         8:intervali{interval}e12:min intervali{min_interval}e5:peers{}:",
        peers_bytes.len()
    )
    .into_bytes();
    out.extend_from_slice(&peers_bytes);
    if !peers6_bytes.is_empty() {
        out.extend_from_slice(format!("6:peers6{}:", peers6_bytes.len()).as_bytes());
        out.extend_from_slice(&peers6_bytes);
    }
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
        assert_eq!(snap.v4.len() + snap.v6.len(), 1); // 排除自己
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
            &[],
            1800,
            600,
        );
        assert!(body.windows(6).any(|w| w == [10, 0, 0, 1, 0xC2, 0x01]));
        assert!(body.starts_with(b"d8:completei1e"));
        assert!(body.ends_with(b"e"));
    }

    #[test]
    fn bencode_announce_ipv6_bep7() {
        let body = bencode_announce(
            1,
            1,
            0,
            &[CompactPeer {
                ip: [10, 0, 0, 1],
                port: 51413,
            }],
            &[CompactPeer6 {
                ip: [
                    0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
                ],
                port: 0xC201,
            }],
            1800,
            600,
        );
        // BEP-7：peers6 紧跟 peers（字节序），18 字节/peer；二进制内容不经 UTF-8 损坏
        assert!(body.windows(6).any(|w| w == [10, 0, 0, 1, 0xC8, 0xD5])); // 51413 = 0xC8D5
        assert!(body.windows(18).any(|w| w[..16]
            == [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]
            && w[16] == 0xC2
            && w[17] == 0x01));
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains("6:peers618:"), "缺 peers6: {s}");
        assert!(
            s.find("5:peers").unwrap() < s.find("6:peers6").unwrap(),
            "字典键序"
        );
    }

    #[test]
    fn bencode_announce_omits_empty_peers6() {
        let body = bencode_announce(1, 2, 3, &[], &[], 1800, 600);
        let s = String::from_utf8(body).unwrap();
        assert!(!s.contains("peers6"), "空 v6 列表不应输出 peers6 键: {s}");
    }

    #[test]
    fn snapshot_splits_v4_v6() {
        let t = PeerTable::new();
        let mut p4 = mk_peer("abc", "p4", 0);
        p4.ip = "192.168.1.2".into();
        let mut p6 = mk_peer("abc", "p6", 0);
        p6.ip = "2001:db8::5".into();
        t.upsert(p4);
        t.upsert(p6);
        let snap = t.snapshot("abc", 50, "");
        assert_eq!(snap.v4.len(), 1);
        assert_eq!(snap.v6.len(), 1);
        assert_eq!(
            snap.v6[0].ip,
            [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]
        );
    }

    #[test]
    fn bencode_announce_includes_interval() {
        let body = bencode_announce(1, 2, 3, &[], &[], 1800, 600);
        let s = String::from_utf8(body).unwrap();
        assert!(s.contains("8:intervali1800e"), "缺 interval: {s}");
        assert!(s.contains("12:min intervali600e"), "缺 min interval: {s}");
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

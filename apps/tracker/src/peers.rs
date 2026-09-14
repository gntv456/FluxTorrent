//! 内存 Peer 表（§5.2 tracker 职责：按 info_hash 分桶 + 桶内 HashMap，90s 超时淘汰）。
//!
//! P0-3（0071）：此前是单张 `DashMap<PeerKey, Peer>`，snapshot/counts 对**全体在线 peer**
//! 全表遍历再按 info_hash 过滤——热门 swarm 一次 announce 就扫全表。现改为
//! `DashMap<info_hash, Swarm>` 二级索引：snapshot/counts 只碰对应桶，复杂度从
//! O(全体 peer) 降到 O(该 swarm)；DashMap 分片锁竞争同时缓解（每桶独立 entry）。

use dashmap::DashMap;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerKey {
    /// info_hash 的 hex 编码（40 字符），与 DB/事件流口径一致
    pub info_hash: String,
    /// peer_id 的 hex 编码
    pub peer_id: String,
}

/// 回连可达性（0071 connectable 抽样）：-1 未测 / 0 不可达 / 1 可达
pub const CONN_UNTESTED: i8 = -1;
pub const CONN_DEAD: i8 = 0;
pub const CONN_OK: i8 = 1;

#[derive(Clone)]
#[allow(dead_code)] // uploaded/downloaded/user_id 供后续审计扩展读取（all_unreachable 等）
pub struct Peer {
    pub key: PeerKey,
    pub ip: String,
    pub port: u16,
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub user_id: i64,
    pub connectable: i8,
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

/// 单个 swarm 的桶：peer_id hex → Peer
#[derive(Default)]
struct Swarm {
    peers: HashMap<String, Peer>,
}

#[derive(Default)]
pub struct PeerTable {
    swarms: DashMap<String, Swarm>,
}

const PEER_TIMEOUT: Duration = Duration::from_secs(90);
/// 0077 分档 TTL（U3D ACTIVE_PEER_TTL 口径）：做种中的 peer 放宽——
/// interval 1800s 下 90s 一刀切会让挂种大户每 90s 全量重建内存表；
/// 3720s = 2×interval+120 冗余，断线种子在两个周期内自然除名。
const SEEDER_TIMEOUT: Duration = Duration::from_secs(3720);
const MAX_PEERS_RESPONSE: usize = 50;

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
    pub fn snapshot(&self, info_hash: &str, numwant: usize, exclude: &str) -> Snapshot {
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
                    CONN_UNTESTED => out.push((p.key.clone(), p.ip.clone(), p.port)),
                    _ => {
                        if retriable.len() < n {
                            retriable.push((p.key.clone(), p.ip.clone(), p.port))
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

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
            connectable: CONN_UNTESTED,
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
    fn swarm_isolation() {
        // P0-3：桶隔离——A swarm 的增删不影响 B swarm，计数互不串扰
        let t = PeerTable::new();
        t.upsert(mk_peer("hot", "p1", 0));
        t.upsert(mk_peer("hot", "p2", 100));
        t.upsert(mk_peer("cold", "p3", 0));
        assert_eq!(t.count_seeders("hot"), 1);
        assert_eq!(t.count_seeders("cold"), 1);
        assert_eq!(t.swarms(), 2);
        t.remove(&PeerKey {
            info_hash: "hot".into(),
            peer_id: "p2".into(),
        });
        assert_eq!(t.swarms(), 2); // cold 桶不受影响
    }

    #[test]
    fn connectable_probe_roundtrip() {
        let t = PeerTable::new();
        t.upsert(mk_peer("abc", "p1", 0));
        t.upsert(mk_peer("def", "p2", 0));
        let probes = t.sample_probes(10);
        assert_eq!(probes.len(), 2); // 未测优先
        t.set_connectable(&probes[0].0, false);
        t.set_connectable(&probes[1].0, true);
        // user 1 在 def swarm 可达 → all_unreachable=false
        assert!(!t.all_unreachable(1));
        t.set_connectable(&probes[1].0, false);
        assert!(t.all_unreachable(1)); // 全部不可达
        let again = t.sample_probes(10);
        assert_eq!(again.len(), 2); // 已测 peer 进入复测轮替
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

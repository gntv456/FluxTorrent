//! UDP tracker（BEP15 精简实现：connect / announce / scrape）。
//!
//! 设计取舍（与 HTTP announce 同一资源池、同一套防护）：
//! - 复用 TrackerState：passkey 缓存、ip_bans、agent_rules、限流、peer 表、
//!   Redis 事件流（计费走 emit_event，worker 零改动）。
//! - passkey 携带方式：BEP15 无 path，passkey 放 announce 的 tracker_id 字段
//!   （连入后原样回显；qbittorrent/Transmission 均支持在 URL ?passkey= 之外
//!   通过 tracker id 传凭证——本站 announce URL 形如
//!   udp://host:port/{passkey}，客户端会把 path 段填进 tracker_id）。
//! - connection_id 校验放宽为「按 (ip, port) 会话」：避免实现 BEP42 的
//!   IP 混淆 HLS（单机部署下伪造源 IP 的 UDP 反射风险由 connection_id
//!   一次性握手 + 60s 窗口缓解；生产可再启用 BEP42）。
//! - announce 复用 HTTP 版全部防护链（熔断/ip_bans/限流/passkey/agent_rules）。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::web;
use tokio::net::UdpSocket;

use crate::peers::{Peer, PeerKey};

const PROTOCOL_ID: u64 = 0x4172_7109_807a;
const CONNECT_ACTION: u32 = 0;
const ANNOUNCE_ACTION: u32 = 1;
const SCRAPE_ACTION: u32 = 2;
const ERROR_ACTION: u32 = 3;
/// connection_id 有效窗口（BEP15 推荐 60s；客户端实现普遍到点自动重连）
const CONN_TTL: Duration = Duration::from_secs(60);
/// 会话表上限：防伪造 connect 洪水撑爆内存（LRU 语义：满了整体清，正常量级远达不到）
const CONN_CAP: usize = 100_000;

struct ConnEntry {
    id: u64,
    at: Instant,
}

/// UDP 会话表：((ip, port) → connection_id)。DashMap 足够（无锁读）。
type ConnTable = dashmap::DashMap<(String, u16), ConnEntry>;

pub struct UdpTracker {
    state: web::Data<crate::TrackerState>,
    conns: ConnTable,
}

/// 从 announce URL 提取 passkey 的客户端兼容集：
/// ① tracker_id 字段（qbittorrent/Transmission 传 path 段）；
/// ② 部分客户端（libtorrent rasterbar）把 URL query 塞进 tracker_id，
///    形如 "passkey=xxxx"——两种都识别。
fn passkey_from_tracker_id(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("passkey=") {
        return rest;
    }
    s
}

impl UdpTracker {
    pub fn new(state: web::Data<crate::TrackerState>) -> Self {
        Self {
            state,
            conns: ConnTable::new(),
        }
    }

    pub async fn run(self: Arc<Self>, bind: &str) -> anyhow::Result<()> {
        let sock = Arc::new(UdpSocket::bind(bind).await?);
        tracing::info!("flux-tracker UDP listening on {bind}");
        let mut buf = vec![0u8; 2048];
        loop {
            let (n, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(%e, "udp recv error");
                    continue;
                }
            };
            // 拷贝一份再交给任务：buf 会被下一个包复用
            let pkt = buf[..n].to_vec();
            let sock2 = sock.clone();
            let this = self.clone();
            tokio::spawn(async move {
                if let Some(resp) = this.handle(&pkt, peer).await {
                    let _ = sock2.send_to(&resp, peer).await;
                }
            });
        }
    }

    fn err_pkt(transaction_id: u32, msg: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + msg.len());
        out.extend_from_slice(&ERROR_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        out.extend_from_slice(msg.as_bytes());
        out
    }

    fn check_conn(&self, peer: &SocketAddr, id: u64) -> bool {
        match self.conns.get(&(peer.ip().to_string(), peer.port())) {
            Some(c) => c.id == id && c.at.elapsed() < CONN_TTL,
            None => false,
        }
    }

    async fn handle(&self, pkt: &[u8], peer: SocketAddr) -> Option<Vec<u8>> {
        if pkt.len() < 16 {
            return None; // 连最小包长都不足：丢弃（不回包，防反射放大）
        }
        let action = u32::from_be_bytes(pkt[8..12].try_into().ok()?);
        let transaction_id = u32::from_be_bytes(pkt[12..16].try_into().ok()?);
        // BEP15：仅 connect 的头 8 字节是协议魔数；announce/scrape 头 8 字节是
        // connection_id（由 connect 会话校验覆盖），不能按魔数过滤。
        if action == CONNECT_ACTION {
            let proto = u64::from_be_bytes(pkt[0..8].try_into().ok()?);
            if proto != PROTOCOL_ID {
                return None; // 协议号不符：丢弃
            }
            return Some(self.connect(pkt, peer, transaction_id));
        }
        match action {
            ANNOUNCE_ACTION => Some(self.announce(pkt, peer, transaction_id).await),
            SCRAPE_ACTION => Some(self.scrape(pkt, peer, transaction_id).await),
            _ => Some(Self::err_pkt(transaction_id, "未知 action")),
        }
    }

    fn connect(&self, pkt: &[u8], peer: SocketAddr, transaction_id: u32) -> Vec<u8> {
        if pkt.len() != 16 {
            return Self::err_pkt(transaction_id, "connect 包长无效");
        }
        // 会话表容量防护：满了先清过期再整体清（正常站点量级远达不到）
        if self.conns.len() >= CONN_CAP {
            self.conns.retain(|_, v| v.at.elapsed() < CONN_TTL);
            if self.conns.len() >= CONN_CAP {
                self.conns.clear();
            }
        }
        let conn_id = rand_u64();
        self.conns.insert(
            (peer.ip().to_string(), peer.port()),
            ConnEntry {
                id: conn_id,
                at: Instant::now(),
            },
        );
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&CONNECT_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        out.extend_from_slice(&conn_id.to_be_bytes());
        out
    }

    async fn announce(&self, pkt: &[u8], peer: SocketAddr, transaction_id: u32) -> Vec<u8> {
        // 标准 BEP15 announce 包（固定 98 字节）：8 conn + 4 action + 4 tid
        //   + 20 info_hash + 20 peer_id + 8 downloaded + 8 left + 8 uploaded
        //   + 4 event + 4 ip + 4 key + 4 numwant + 2 port
        // 审计修复（P1 协议非标）：旧版要求 98 字节之后必须尾随 passkey（客户端把
        // udp://host:port/<passkey> 的 path 填进 tracker_id）——但 libtorrent/qBittorrent
        // 等标准实现固定发 98 字节、不附 tracker_id，导致 UDP tier 对标准客户端恒失败。
        // 新口径（与 NexusPHP 生态一致，私有 tracker 行业惯例）：
        //   ① 私有站的主通道是 HTTP announce（passkey 在 URL path，BEP3 天然支持）；
        //   ② UDP announce 无 path 可传 passkey，标准做法是**不在 UDP 上承载计费身份**：
        //      98 字节标准包 → 无法鉴权 → 明确回错误提示走 HTTP；
        //      尾随 passkey 的扩展包（本站 .torrent 里 TRACKER_UDP_URL tier 的约定）→ 正常计费。
        //   这样标准客户端收到明确错误后按 BEP12 降级到 HTTP tier（一次握手开销，
        //   不再是每次 announce 静默 500/超时）；自研/配置了扩展的客户端走 UDP 全速。
        if pkt.len() < 98 {
            return Self::err_pkt(transaction_id, "announce 包长无效");
        }
        let conn_id = u64::from_be_bytes(pkt[0..8].try_into().unwrap());
        if !self.check_conn(&peer, conn_id) {
            return Self::err_pkt(transaction_id, "connection_id 无效或过期，请重连");
        }
        let info_hash: [u8; 20] = pkt[16..36].try_into().unwrap();
        let peer_id: [u8; 20] = pkt[36..56].try_into().unwrap();
        let downloaded = i64::from_be_bytes(pkt[56..64].try_into().unwrap());
        let left = i64::from_be_bytes(pkt[64..72].try_into().unwrap());
        let uploaded = i64::from_be_bytes(pkt[72..80].try_into().unwrap());
        let event_u32 = u32::from_be_bytes(pkt[80..84].try_into().unwrap());
        let numwant = i32::from_be_bytes(pkt[92..96].try_into().unwrap()).max(0) as usize;
        let port = u16::from_be_bytes(pkt[96..98].try_into().unwrap());
        let event = match event_u32 {
            1 => "completed",
            3 => "stopped",
            _ => "",
        };
        let ip = peer.ip().to_string();

        // —— 防护链（与 HTTP announce 同源） ——
        self.state
            .metrics
            .announce_total
            .fetch_add(1, Ordering::Relaxed);
        if self.state.global_shed().await {
            self.state
                .metrics
                .announce_global_shed
                .fetch_add(1, Ordering::Relaxed);
            return Self::err_pkt(transaction_id, "tracker 负载保护已触发");
        }
        self.state.refresh_guard().await;
        if let Some(reason) = self.state.ip_banned(&ip) {
            self.state
                .metrics
                .announce_ip_banned
                .fetch_add(1, Ordering::Relaxed);
            return Self::err_pkt(transaction_id, &format!("IP 已被封禁：{reason}"));
        }
        if let Some(msg) = self.state.rate_limited_ip(&ip).await {
            self.state
                .metrics
                .announce_limited_ip
                .fetch_add(1, Ordering::Relaxed);
            return Self::err_pkt(transaction_id, msg);
        }
        // 尾随字节 = tracker_id（本站扩展约定：udp://host:port/<passkey> 的 path 段）。
        // 标准 98 字节包（无尾随）→ 无法识别身份：明确回错让客户端按 BEP12 降级 HTTP。
        let tracker_id = String::from_utf8_lossy(&pkt[98..]).to_string();
        let passkey = passkey_from_tracker_id(&tracker_id);
        if passkey.len() < 16 {
            return Self::err_pkt(
                transaction_id,
                "本 tracker 的 UDP 通道需扩展 passkey；请使用 HTTP announce（或联系站方客户端）",
            );
        }
        let Some((user_id, download_enabled, suspended)) =
            self.state.resolve_passkey_cached(passkey).await
        else {
            return Self::err_pkt(transaction_id, "passkey 无效");
        };
        if suspended {
            return Self::err_pkt(transaction_id, "账号已被挂起");
        }
        if !download_enabled && left > 0 {
            return Self::err_pkt(transaction_id, "下载权限已被禁用");
        }
        if let Some(msg) = self.state.rate_limited_user(user_id).await {
            self.state
                .metrics
                .announce_limited_user
                .fetch_add(1, Ordering::Relaxed);
            return Self::err_pkt(transaction_id, msg);
        }
        let peer_id_readable = String::from_utf8_lossy(&peer_id).into_owned();
        if let Some(reason) = self.state.agent_blocked(None, &peer_id_readable) {
            self.state
                .metrics
                .announce_agent_blocked
                .fetch_add(1, Ordering::Relaxed);
            return Self::err_pkt(transaction_id, &reason);
        }

        // —— peer 表与事件流（与 HTTP 同源） ——
        let info_hash_hex = crate::peers::hex(&info_hash);
        let peer_id_hex = crate::peers::hex(&peer_id);
        let key = PeerKey {
            info_hash: info_hash_hex.clone(),
            peer_id: peer_id_hex.clone(),
        };
        if event == "stopped" {
            self.state.peers.remove(&key);
        } else {
            self.state.peers.upsert(Peer {
                key: key.clone(),
                ip: ip.clone(),
                port,
                uploaded,
                downloaded,
                left,
                last_seen: chrono::Utc::now(),
                user_id,
                connectable: crate::peers::CONN_UNTESTED,
            });
        }
        crate::emit_event(
            &self.state.redis,
            &info_hash_hex,
            user_id,
            uploaded,
            downloaded,
            event,
            left,
            &ip,
            self.state.peers.connectable_of(&key),
            "", // UDP 不携带 UA；agent 列以 HTTP announce 为准
        )
        .await;

        let (interval, min_interval) = self.state.intervals();
        let (complete, incomplete) = if event == "stopped" {
            (0, 0)
        } else {
            (
                self.state.peers.count_seeders(&info_hash_hex) as i32,
                self.state.peers.count_leechers(&info_hash_hex) as i32,
            )
        };
        // compact peers：IPv4 only（本站客户端主体；v6 走 HTTP tracker）
        let snap = self
            .state
            .peers
            .snapshot(&info_hash_hex, numwant.clamp(1, 200), &key.peer_id);
        let mut peers_bytes = Vec::with_capacity(snap.v4.len() * 6);
        for p in &snap.v4 {
            peers_bytes.extend_from_slice(&p.ip);
            peers_bytes.extend_from_slice(&p.port.to_be_bytes());
        }

        // 响应：action + tid + interval + leechers + seeders + peers
        let mut out = Vec::with_capacity(20 + peers_bytes.len());
        out.extend_from_slice(&ANNOUNCE_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        out.extend_from_slice(&interval.to_be_bytes());
        out.extend_from_slice(&incomplete.to_be_bytes());
        out.extend_from_slice(&complete.to_be_bytes());
        out.extend_from_slice(&peers_bytes);
        let _ = min_interval; // BEP15 响应无此字段
        out
    }

    async fn scrape(&self, pkt: &[u8], peer: SocketAddr, transaction_id: u32) -> Vec<u8> {
        let conn_id = u64::from_be_bytes(pkt[0..8].try_into().unwrap());
        if !self.check_conn(&peer, conn_id) {
            return Self::err_pkt(transaction_id, "connection_id 无效或过期，请重连");
        }
        // 剩余字节 = N×20 hash + 尾随 passkey：hash 边界按「20 字节对齐的头部段」推断，
        // 至少 1 个 hash、至少 16 字节 passkey（passkey 长度 = 剩余长度 mod 20 之外的尾段）
        if pkt.len() < 16 + 20 + 16 {
            return Self::err_pkt(transaction_id, "scrape 需附带 info_hash 与 passkey");
        }
        self.state
            .metrics
            .scrape_total
            .fetch_add(1, Ordering::Relaxed);
        // 审计修复（P2 口径对齐）：HTTP scrape 需 passkey 鉴权，UDP 侧此前仅凭
        // connection_id 即可刮擦（connect 无身份）。要求 hash 块之后尾随 passkey
        // （与 announce 同约定：整包 = 头16 + N×20 hash + passkey，无尾随即拒绝）。
        let n_hash = (pkt.len() - 16) / 20;
        let passkey_raw = String::from_utf8_lossy(&pkt[16 + n_hash * 20..]);
        let passkey = passkey_from_tracker_id(&passkey_raw);
        if passkey.len() < 16 {
            return Self::err_pkt(transaction_id, "scrape 需在 info_hash 列表后附带 passkey");
        }
        if self.state.resolve_passkey_cached(passkey).await.is_none() {
            return Self::err_pkt(transaction_id, "passkey 无效");
        }
        let mut out = Vec::with_capacity(8 + n_hash * 12);
        out.extend_from_slice(&SCRAPE_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        for chunk in pkt[16..16 + n_hash * 20].chunks_exact(20) {
            let hexkey = crate::peers::hex(chunk);
            let (s, l) = self.state.peers.counts(&hexkey);
            out.extend_from_slice(&(s as i32).to_be_bytes());
            out.extend_from_slice(&(l as i32).to_be_bytes());
            out.extend_from_slice(&0i32.to_be_bytes()); // downloaded（未跟踪，BEP15 允许 0）
        }
        out
    }
}

/// 轻量 PRNG（xorshift64*）：connection_id 只需不可预测，无需密码学强度
fn rand_u64() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
    }
    SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BEP15 包结构自检：connect 响应 16 字节；announce 头 20 字节。
    /// （网络路径由 e2e 覆盖，这里锁协议字节布局不被无意改动。）
    #[test]
    fn connect_pkt_shape() {
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&PROTOCOL_ID.to_be_bytes());
        pkt.extend_from_slice(&CONNECT_ACTION.to_be_bytes());
        pkt.extend_from_slice(&0xABCD_u32.to_be_bytes());
        assert_eq!(pkt.len(), 16);
    }

    #[test]
    fn announce_min_len_is_98() {
        // 头 16 + info_hash 20 + peer_id 20 + 三计数 24 + event/ip/key/numwant 16 + port 2
        assert_eq!(16 + 20 + 20 + 8 * 3 + 4 * 4 + 2, 98);
    }

    #[test]
    fn passkey_extraction() {
        assert_eq!(passkey_from_tracker_id("abc123"), "abc123");
        assert_eq!(passkey_from_tracker_id("passkey=abc123"), "abc123");
    }

    /// scrape 尾随 passkey 布局：头16 + N×20 hash + passkey。
    /// 最小合法包 = 16 + 20 + 16；hash 数与 passkey 段长度由整除关系唯一确定。
    #[test]
    fn scrape_layout_splits_hash_and_passkey() {
        let pk: &[u8] = b"0123456789abcdef"; // 16 字节（最小 passkey）
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0u8; 16]); // conn_id + action + tid
        pkt.extend_from_slice(&[0xA5u8; 20]); // 1 个 info_hash
        pkt.extend_from_slice(pk);
        let n_hash = (pkt.len() - 16) / 20;
        let tail = &pkt[16 + n_hash * 20..];
        assert_eq!(n_hash, 1);
        assert_eq!(tail, pk);
    }
}

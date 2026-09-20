//! UDP announce 处理（BEP15）：解析、防护链、peer 表更新、事件投递与响应编码。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use crate::peers::{Peer, PeerKey};

use super::core::UdpTracker;
use super::pkt::{passkey_from_tracker_id, ANNOUNCE_ACTION};

pub(super) async fn announce(
    t: &UdpTracker,
    pkt: &[u8],
    peer: SocketAddr,
    transaction_id: u32,
) -> Vec<u8> {
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
        return UdpTracker::err_pkt(transaction_id, "announce 包长无效");
    }
    let conn_id = u64::from_be_bytes(pkt[0..8].try_into().unwrap());
    if !t.check_conn(&peer, conn_id) {
        return UdpTracker::err_pkt(
            transaction_id,
            "connection_id 无效或过期，请重连",
        );
    }
    let info_hash: [u8; 20] = pkt[16..36].try_into().unwrap();
    let peer_id: [u8; 20] = pkt[36..56].try_into().unwrap();
    let downloaded = i64::from_be_bytes(pkt[56..64].try_into().unwrap());
    let left = i64::from_be_bytes(pkt[64..72].try_into().unwrap());
    let uploaded = i64::from_be_bytes(pkt[72..80].try_into().unwrap());
    let event_u32 = u32::from_be_bytes(pkt[80..84].try_into().unwrap());
    let numwant =
        i32::from_be_bytes(pkt[92..96].try_into().unwrap()).max(0) as usize;
    let port = u16::from_be_bytes(pkt[96..98].try_into().unwrap());
    let event = match event_u32 {
        1 => "completed",
        3 => "stopped",
        _ => "",
    };
    let ip = peer.ip().to_string();

    // —— 防护链（与 HTTP announce 同源） ——
    t.state
        .metrics
        .announce_total
        .fetch_add(1, Ordering::Relaxed);
    if t.state.global_shed().await {
        t.state
            .metrics
            .announce_global_shed
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(transaction_id, "tracker 负载保护已触发");
    }
    t.state.refresh_guard().await;
    if let Some(reason) = t.state.ip_banned(&ip) {
        t.state
            .metrics
            .announce_ip_banned
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(
            transaction_id,
            &format!("IP 已被封禁：{reason}"),
        );
    }
    if let Some(msg) = t.state.rate_limited_ip(&ip).await {
        t.state
            .metrics
            .announce_limited_ip
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(transaction_id, msg);
    }
    // 尾随字节 = tracker_id（本站扩展约定：udp://host:port/<passkey> 的 path 段）。
    // 标准 98 字节包（无尾随）→ 无法识别身份：明确回错让客户端按 BEP12 降级 HTTP。
    let tracker_id = String::from_utf8_lossy(&pkt[98..]).to_string();
    let passkey = passkey_from_tracker_id(&tracker_id);
    if passkey.len() < 16 {
        return UdpTracker::err_pkt(
            transaction_id,
            "本 tracker 的 UDP 通道需扩展 passkey；请使用 HTTP announce（或联系站方客户端）",
        );
    }
    let Some((user_id, download_enabled, suspended)) =
        t.state.resolve_passkey_cached(passkey).await
    else {
        return UdpTracker::err_pkt(transaction_id, "passkey 无效");
    };
    if suspended {
        return UdpTracker::err_pkt(transaction_id, "账号已被挂起");
    }
    if !download_enabled && left > 0 {
        return UdpTracker::err_pkt(transaction_id, "下载权限已被禁用");
    }
    if let Some(msg) = t.state.rate_limited_user(user_id).await {
        t.state
            .metrics
            .announce_limited_user
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(transaction_id, msg);
    }
    let peer_id_readable = String::from_utf8_lossy(&peer_id).into_owned();
    if let Some(reason) = t.state.agent_blocked(None, &peer_id_readable) {
        t.state
            .metrics
            .announce_agent_blocked
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(transaction_id, &reason);
    }

    // —— peer 表与事件流（与 HTTP 同源） ——
    let info_hash_hex = crate::peers::hex(&info_hash);
    let peer_id_hex = crate::peers::hex(&peer_id);
    let key = PeerKey {
        info_hash: info_hash_hex.clone(),
        peer_id: peer_id_hex.clone(),
    };
    if event == "stopped" {
        t.state.peers.remove(&key);
    } else {
        t.state.peers.upsert(Peer {
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
        &t.state.redis,
        &info_hash_hex,
        user_id,
        uploaded,
        downloaded,
        event,
        left,
        &ip,
        t.state.peers.connectable_of(&key),
        "", // UDP 不携带 UA；agent 列以 HTTP announce 为准
    )
    .await;

    let (interval, min_interval) = t.state.intervals();
    let (complete, incomplete) = if event == "stopped" {
        (0, 0)
    } else {
        (
            t.state.peers.count_seeders(&info_hash_hex) as i32,
            t.state.peers.count_leechers(&info_hash_hex) as i32,
        )
    };
    // compact peers：IPv4 only（本站客户端主体；v6 走 HTTP tracker）
    let snap = t.state.peers.snapshot(
        &info_hash_hex,
        numwant.clamp(1, 200),
        &key.peer_id,
    );
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

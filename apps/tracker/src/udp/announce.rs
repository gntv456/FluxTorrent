//! UDP announce 处理（BEP15）：解析、防护链、peer 表更新、事件投递与响应编码。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use crate::peers::PeerKey;

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
    // 审计 10-06 第 1 条：UDP 侧对 left<0 曾原样透传（is_seeder=left==0 判种
    // 侥幸不中，但负数 left 进事件流/计费口径未定义）——与 HTTP 同口径拒绝。
    if left < 0 || uploaded < 0 || downloaded < 0 {
        return UdpTracker::err_pkt(transaction_id, "统计数值无效");
    }
    let event_u32 = u32::from_be_bytes(pkt[80..84].try_into().unwrap());
    // numwant=0 合法（明确不要 peer 列表）；负数按 0
    let numwant =
        i32::from_be_bytes(pkt[92..96].try_into().unwrap()).max(0) as usize;
    let port = u16::from_be_bytes(pkt[96..98].try_into().unwrap());
    // BEP15 事件码：0=none 1=completed 2=started 3=stopped。
    // 旧版漏了 2 ⇒ UDP 通道永远不产生 started 事件（审计 10-07 P3）。
    let event = match event_u32 {
        1 => "completed",
        2 => "started",
        3 => "stopped",
        _ => "",
    };
    // 端口口径（特权端口 + 服务端口黑名单）不在这里重复一份，
    // 统一由 gate::announce_gate 判（五轮实测：本地抄一份的两条通道
    // 各自演化，正是黑名单一直没生效的那半边）。
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
    let Some((user_id, download_enabled, suspended, class_id)) =
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
    // 种子白名单（P0-2，与 HTTP announce 同口径；UDP 扩展 passkey 鉴权之后）
    let info_hash_hex = crate::peers::hex(&info_hash);
    if !t.state.torrent_registered(&info_hash_hex).await {
        t.state
            .metrics
            .announce_torrent_unknown
            .fetch_add(1, Ordering::Relaxed);
        return UdpTracker::err_pkt(
            transaction_id,
            "种子不存在或不可用（torrent unregistered）",
        );
    }

    // —— peer 表与事件流（与 HTTP 同源） ——
    //
    // 准入判定必须走同一个 gate：五轮实测发现 UDP 这条路整个绕过了
    // `gate::announce_gate` —— ① HTTP 侧拒 `left > 种子大小`（假 announce），
    // UDP 侧照收并进事件流；② HTTP 侧待审种子对局外人清空 peer 与计数，
    // UDP 侧照样把发布者的 ip:port 发给任何人。同一份 swarm 数据两个入口，
    // 严的那个形同虚设——攻击者只要换个协议就打通整条防线。
    let peer_id_hex = crate::peers::hex(&peer_id);
    let hide_peers = match crate::http_track::gate::announce_gate(
        &t.state,
        &info_hash_hex,
        user_id,
        class_id,
        left,
        port,
        event == "stopped",
    ) {
        Ok(hide) => hide,
        Err(msg) => return UdpTracker::err_pkt(transaction_id, msg),
    };
    let key = PeerKey {
        info_hash: info_hash_hex.clone(),
        peer_id: peer_id_hex.clone(),
    };
    // 0227 G31 审查遗留#1：外置模式（FLUX_TRACKER_PEER_STORE=redis）下 UDP
    // 与 HTTP 同读写 Redis swarm——此前 UDP 只进本进程内存，多副本/双协议下
    // peer 视图分裂。内存表仍同步维护（快照导出/降级路径不变）。
    // stopped 归属校验 + 顶号拒写也统一走 gate::write_peer（旧写法在这里
    // 自己抄了一份 upsert/remove，与 HTTP 侧各自演化）。
    if let Err(msg) = crate::http_track::gate::write_peer(
        &t.state, &key, &ip, event, port, uploaded, downloaded, left, user_id,
    )
    .await
    {
        return UdpTracker::err_pkt(transaction_id, msg);
    }
    let connectable = if crate::peers::external::external_enabled() {
        let mut r = t.state.redis.clone();
        crate::peers::external::connectable_of(&mut r, &key).await
    } else {
        t.state.peers.connectable_of(&key)
    };
    // 事件合并（P2-1，与 HTTP 侧同语义）：周期 announce 触窗则不发事件，
    // started/completed/stopped 不受影响。
    let merged = event.is_empty()
        && !t.state.claim_event_window(user_id, &info_hash_hex).await;
    if merged {
        t.state
            .metrics
            .announce_evt_merged
            .fetch_add(1, Ordering::Relaxed);
    } else {
        crate::emit_event(
            &t.state.redis,
            &info_hash_hex,
            user_id,
            uploaded,
            downloaded,
            event,
            left,
            &ip,
            connectable,
            "", // UDP 不携带 UA；agent 列以 HTTP announce 为准
            port,
        )
        .await;
    }

    // 下发的 interval 与上面判快发用的是同一个值（gate::peer_interval）
    let interval = crate::http_track::limits::peer_interval(
        &t.state,
        &info_hash_hex,
        user_id,
    );
    // hide_peers：待审种子对局外人清空计数与 peer 列表（与 HTTP 侧
    // reply::announce_body 的 `hide` 同一语义）
    let (complete, incomplete, snap) = if event == "stopped" || hide_peers {
        (0, 0, Default::default())
    } else if crate::peers::external::external_enabled() {
        let mut r = t.state.redis.clone();
        let (seeders, leechers) =
            crate::peers::external::counts(&mut r, &info_hash_hex).await;
        let snap = crate::peers::external::snapshot(
            &mut r,
            &info_hash_hex,
            numwant.clamp(0, 200),
            user_id,
        )
        .await;
        (seeders as i32, leechers as i32, snap)
    } else {
        let snap = t.state.peers.snapshot(
            &info_hash_hex,
            numwant.clamp(0, 200),
            user_id,
        );
        (
            t.state.peers.count_seeders(&info_hash_hex) as i32,
            t.state.peers.count_leechers(&info_hash_hex) as i32,
            snap,
        )
    };
    let mut peers_bytes = Vec::with_capacity(snap.v4.len() * 6);
    for p in &snap.v4 {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }

    // 响应：action + tid + interval + leechers + seeders + peers
    let mut out = Vec::with_capacity(20 + peers_bytes.len());
    out.extend_from_slice(&ANNOUNCE_ACTION.to_be_bytes());
    out.extend_from_slice(&transaction_id.to_be_bytes());
    // 审查/V17 修正：intervals() 返回 i64，直接 to_be_bytes 是 8 字节——
    // BEP-15 响应的 interval/leechers/seeders 各 4 字节，超写会令标准客户端
    // 解析错位（响应 24 字节畸形）。
    out.extend_from_slice(&(interval as i32).to_be_bytes());
    out.extend_from_slice(&(incomplete as i32).to_be_bytes());
    out.extend_from_slice(&(complete as i32).to_be_bytes());
    out.extend_from_slice(&peers_bytes);
    out
}

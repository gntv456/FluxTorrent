//! UDP scrape 处理（BEP15）：connection_id 校验、passkey 鉴权与计数响应编码。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use super::core::UdpTracker;
use super::pkt::{passkey_from_tracker_id, SCRAPE_ACTION};

pub(super) async fn scrape(
    t: &UdpTracker,
    pkt: &[u8],
    peer: SocketAddr,
    transaction_id: u32,
) -> Vec<u8> {
    let conn_id = u64::from_be_bytes(pkt[0..8].try_into().unwrap());
    if !t.check_conn(&peer, conn_id) {
        return UdpTracker::err_pkt(
            transaction_id,
            "connection_id 无效或过期，请重连",
        );
    }
    // 剩余字节 = N×20 hash + 尾随 passkey：hash 边界按「20 字节对齐的头部段」推断，
    // 至少 1 个 hash、至少 16 字节 passkey（passkey 长度 = 剩余长度 mod 20 之外的尾段）
    if pkt.len() < 16 + 20 + 16 {
        return UdpTracker::err_pkt(
            transaction_id,
            "scrape 需附带 info_hash 与 passkey",
        );
    }
    t.state.metrics.scrape_total.fetch_add(1, Ordering::Relaxed);
    // 审计修复（P2 口径对齐）：HTTP scrape 需 passkey 鉴权，UDP 侧此前仅凭
    // connection_id 即可刮擦（connect 无身份）。要求 hash 块之后尾随 passkey
    // （与 announce 同约定：整包 = 头16 + N×20 hash + passkey，无尾随即拒绝）。
    let n_hash = (pkt.len() - 16) / 20;
    let passkey_raw = String::from_utf8_lossy(&pkt[16 + n_hash * 20..]);
    let passkey = passkey_from_tracker_id(&passkey_raw);
    if passkey.len() < 16 {
        return UdpTracker::err_pkt(
            transaction_id,
            "scrape 需在 info_hash 列表后附带 passkey",
        );
    }
    if t.state.resolve_passkey_cached(passkey).await.is_none() {
        return UdpTracker::err_pkt(transaction_id, "passkey 无效");
    }
    let mut out = Vec::with_capacity(8 + n_hash * 12);
    out.extend_from_slice(&SCRAPE_ACTION.to_be_bytes());
    out.extend_from_slice(&transaction_id.to_be_bytes());
    for chunk in pkt[16..16 + n_hash * 20].chunks_exact(20) {
        let hexkey = crate::peers::hex(chunk);
        let (s, l) = t.state.peers.counts(&hexkey);
        out.extend_from_slice(&(s as i32).to_be_bytes());
        out.extend_from_slice(&(l as i32).to_be_bytes());
        out.extend_from_slice(&0i32.to_be_bytes()); // downloaded（BEP15 允许 0）
    }
    out
}

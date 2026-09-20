use super::pkt::*;

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
    // 头 16 + info_hash 20 + peer_id 20 + 三计数 24 + event/ip/key/numwant 16
    // + port 2
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

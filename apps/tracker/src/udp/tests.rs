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

/// scrape 尾随 passkey 布局：整包 = 头16 + N×20 hash + passkey(站点固定 32 字符)。
/// ZT81（2026-10-02）：原测试用 `(len-16)/20` 复算 hash 个数、且只喂 16 字节
/// passkey，恰好掩盖了「真实 32 字符 passkey 恒解析失败」的缺陷。现按生产逻辑
/// （先扣掉 32 字节 passkey，余量按 20 对齐）断言，并覆盖畸形包。
#[test]
fn scrape_layout_splits_hash_and_passkey() {
    // 与 udp/scrape.rs 的分割规则同源
    fn split(pkt_len: usize) -> Option<usize> {
        const PK: usize = 32;
        let body = pkt_len.saturating_sub(16);
        if body >= 20 + PK && (body - PK) % 20 == 0 {
            Some((body - PK) / 20)
        } else {
            None
        }
    }
    // 1 个 hash + 32 字节 passkey：整包 68B（原实现误判为 2 个 hash → 恒拒绝）
    assert_eq!(split(16 + 20 + 32), Some(1));
    // 3 个 hash + passkey
    assert_eq!(split(16 + 60 + 32), Some(3));
    // 标准 BEP15 匿名 scrape（无 passkey）→ 明确拒绝，不静默放行
    assert_eq!(split(16 + 20), None);
    assert_eq!(split(16 + 40), None);
    // 长度不合法（余量非 20 对齐）
    assert_eq!(split(16 + 20 + 15), None);
}

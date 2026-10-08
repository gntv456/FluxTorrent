//! 协议编码侧用例（从 peers/tests.rs 拆出，300 行门禁）。
//!
//! 覆盖 bencode announce 响应的字节完整性 / BEP7 v6 / 字典键序 /
//! 非 compact 列表，以及 peer_id、percent-decode、hex 三个纯函数。

use crate::peers::{
    bencode_announce, hex, peer_id_bytes, percent_decode, CompactPeer,
    CompactPeer6,
};

#[test]
fn bencode_announce_binary_integrity() {
    let body = bencode_announce(
        1,
        2,
        3,
        &[CompactPeer {
            ip: [10, 0, 0, 1],
            port: 0xC201, // 高字节非 ASCII —— 验证不经 UTF-8 损坏
            peer_id: [0u8; 20],
        }],
        &[],
        1800,
        600,
        true,
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
            peer_id: [0u8; 20],
        }],
        &[CompactPeer6 {
            ip: [
                0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
            ],
            port: 0xC201,
        }],
        1800,
        600,
        true,
    );
    // BEP-7：peers6 紧跟 peers（字节序），18 字节/peer；二进制内容不经 UTF-8 损坏
    // 51413 = 0xC8D5
    assert!(body.windows(6).any(|w| w == [10, 0, 0, 1, 0xC8, 0xD5]));
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
    let body = bencode_announce(1, 2, 3, &[], &[], 1800, 600, true);
    let s = String::from_utf8(body).unwrap();
    assert!(!s.contains("peers6"), "空 v6 列表不应输出 peers6 键: {s}");
}

/// BEP3 要求字典键按原始字节序排列。旧实现写成 complete/incomplete/downloaded，
/// 宽容客户端能忍但严格校验器会拒收 —— 0267 修正后必须锁死这个顺序。
#[test]
fn bencode_announce_dict_key_order_is_byte_sorted() {
    let body = bencode_announce(1, 2, 3, &[], &[], 1800, 600, true);
    let s = String::from_utf8(body).unwrap();
    let pos = |k: &str| s.find(k).unwrap_or_else(|| panic!("缺 {k}"));
    let (c, dl, inc, itv, min, p) = (
        pos("8:complete"),
        pos("10:downloaded"),
        pos("10:incomplete"),
        pos("8:interval"),
        pos("12:min interval"),
        pos("5:peers"),
    );
    assert!(
        c < dl && dl < inc && inc < itv && itv < min && min < p,
        "键序不合 BEP3: {s}"
    );
}

/// 非 compact 回退：peers 必须是字典列表（老客户端），且 peer id 原样透传
#[test]
fn bencode_announce_non_compact_dict_list() {
    let mut pid = [0u8; 20];
    pid[..8].copy_from_slice(b"-qB4650-");
    let body = bencode_announce(
        1,
        0,
        0,
        &[CompactPeer {
            ip: [192, 168, 1, 9],
            port: 6881,
            peer_id: pid,
        }],
        &[],
        1800,
        600,
        false,
    );
    // 列表而非字节串
    assert!(
        body.windows(8).any(|w| w == b"5:peersl"),
        "非 compact 的 peers 应为列表"
    );
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("d2:ip11:192.168.1.9"), "应发 IP 字符串: {s}");
    assert!(s.contains("7:peer id20:"), "应含 20 字节 peer id: {s}");
    assert!(s.contains("4:porti6881ee"), "应含端口: {s}");
    // 非 compact 不发 peers6（那些客户端读不懂 BEP7）
    assert!(!s.contains("peers6"), "非 compact 不应发 peers6");
    assert!(body.windows(8).any(|w| w == b"-qB4650-"));
}

/// peer_id 解码容错：脏数据不能把 tracker 打挂
#[test]
fn peer_id_bytes_tolerates_junk() {
    assert_eq!(peer_id_bytes(""), [0u8; 20]);
    assert_eq!(peer_id_bytes("zz"), [0u8; 20]);
    assert_eq!(peer_id_bytes("0102"), {
        let mut e = [0u8; 20];
        e[0] = 1;
        e[1] = 2;
        e
    });
    // 超长（>40 hex）只取前 20 字节
    assert_eq!(peer_id_bytes(&"ff".repeat(50)), [0xffu8; 20]);
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

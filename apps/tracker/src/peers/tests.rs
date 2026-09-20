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

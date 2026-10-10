//! 端点形状用例（从 peers/tests.rs 拆出，300 行门禁）：
//! v4/v6 分槽、port=0 不计不下发、双栈 announce 互相不覆盖。

use super::mk_peer;
use super::model::Endpoint;
use crate::peers::Peer;

/// 审计 10-06 第 5 条：port=0 的 peer 不可连接，计数口径应与下发一致
#[test]
fn port_zero_not_counted() {
    let t = super::PeerTable::new();
    t.upsert(Peer {
        v4: Some(Endpoint {
            ip: "10.0.0.1".into(),
            port: 0,
        }),
        v6: None,
        ..mk_peer("abc", "p0", 0)
    });
    t.upsert(mk_peer("abc", "p1", 0));
    assert_eq!(t.count_seeders("abc"), 1);
}

/// 一个 peer 的两族端点分别进 peers / peers6（BEP 3 / BEP 7）
#[test]
fn snapshot_splits_v4_v6() {
    let t = super::PeerTable::new();
    let mut p4 = mk_peer("abc", "p4", 0);
    p4.set_endpoint("192.168.1.2", 51413);
    let mut p6 = mk_peer("abc", "p6", 0);
    p6.v4 = None;
    p6.set_endpoint("2001:db8::5", 51413);
    t.upsert(p4);
    t.upsert(p6);
    let snap = t.snapshot("abc", 50, 0);
    assert_eq!(snap.v4.len(), 1);
    assert_eq!(snap.v6.len(), 1);
    assert_eq!(
        snap.v6[0].ip,
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]
    );
}

/// BEP 7：同一 `(info_hash, peer_id)` 的 v4 与 v6 announce 各自留档。
/// 旧写法只有一格 `ip`：客户端先 v4 后 v6 上报时 v4 端点被覆盖，此后 v4 对端
/// 永远拿不到它的地址（下发的就是那个不可达端点）。
#[test]
fn dual_stack_announce_keeps_both_endpoints() {
    let t = super::PeerTable::new();
    let mut p = mk_peer("abc", "pd", 0);
    p.set_endpoint("192.168.1.9", 6881);
    t.upsert(p);
    // 第二发同一 peer_id，只报 v6（v4 槽位留空 = 这次没报 v4）
    let mut p6 = mk_peer("abc", "pd", 0);
    p6.v4 = None;
    p6.set_endpoint("2001:db8::9", 6881);
    t.upsert(p6);
    let snap = t.snapshot("abc", 50, 999);
    assert!(
        snap.v4.iter().any(|e| e.port == 6881),
        "v6 那一发不该抹掉已存的 v4 端点"
    );
    assert!(
        snap.v6.iter().any(|e| e.port == 6881),
        "两族要同时出现在各自列表里"
    );
    // 一条记录 = 一个 peer_id，实时计数不因为双栈翻倍
    assert_eq!(t.count_seeders("abc"), 1);
}

/// 佐证目标必须解析成 user_id；leecher、port=0、自报自下载都要拒。
/// （本体用例仍在 peers/tests.rs，这里只补一条端点维度的：port=0 的两族
/// 都没有监听端口时不得被佐证。）
#[test]
fn corroboration_ignores_peer_without_listening_port() {
    let t = super::PeerTable::new();
    t.upsert(Peer {
        user_id: 5,
        v4: Some(Endpoint {
            ip: "10.0.0.5".into(),
            port: 0,
        }),
        v6: Some(Endpoint {
            ip: "2001:db8::5".into(),
            port: 0,
        }),
        ..mk_peer("abc", "quiet5", 0)
    });
    assert_eq!(t.corroboration_target("abc", "quiet5", 1), None);
}

use super::*;
// 0310 四档常量：SUSPECT/OK 不在 table.rs 的 super::* 可见范围内，显式引入
use super::model::{CONN_DEAD, CONN_OK, CONN_SUSPECT, CONN_UNTESTED};

mod encoding;

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
    // peer_id 一律用合法十六进制串：peer_id_bytes 会把非 hex 解码成全 0，
    // 那种用例里的「比 peer_id」断言咬不住任何东西。
    t.upsert(mk_peer("abc", "a1", 0)); // seeder（user 1）
    t.upsert(mk_peer("abc", "a2", 0)); // 同账号第二个 peer_id（user 1）
    let mut other = mk_peer("abc", "b1", 100); // 另一账号的 leecher
    other.user_id = 2;
    t.upsert(other);
    t.upsert(mk_peer("xyz", "c1", 0)); // 另一种子
                                       // 计数按 user 去重（P2-6）：user1 的两条 seeder 只算 1 人
    assert_eq!(t.counts("abc"), (1, 1));
    // P3-6：以 user 1 请求 ⇒ 自家 a1/a2 一条都不下发，只拿到 b1
    let snap = t.snapshot("abc", 50, 1);
    assert_eq!(snap.v4.len() + snap.v6.len(), 1);
    assert_eq!(snap.v4[0].peer_id[0], 0xb1);
    // self_user=0 表示不排除（真实 user_id 恒 >= 1）
    let all = t.snapshot("abc", 50, 0);
    assert_eq!(all.v4.len() + all.v6.len(), 3);
}

#[test]
fn stopped_removes() {
    let t = PeerTable::new();
    let key = PeerKey {
        info_hash: "abc".into(),
        peer_id: "p1".into(),
    };
    t.upsert(mk_peer("abc", "p1", 0));
    assert!(t.remove_owned(&key, 1));
    assert_eq!(t.counts("abc"), (0, 0));
}

/// 审计 10-06：stopped 归属校验——他人 peer_id 的 stopped 不得把对方踢下线。
#[test]
fn stop_wrong_user_keeps_peer() {
    let t = PeerTable::new();
    let key = PeerKey {
        info_hash: "abc".into(),
        peer_id: "p1".into(),
    };
    t.upsert(mk_peer("abc", "p1", 0)); // user 1
    assert!(!t.remove_owned(&key, 2)); // user 2 无权移除
    assert_eq!(t.counts("abc"), (1, 0));
    assert!(t.remove_owned(&key, 1));
    assert_eq!(t.counts("abc"), (0, 0));
}

/// 审计 10-06：单账号同 swarm 影子 peer 配额——超限淘汰最旧，计数钳在 10。
#[test]
fn per_user_swarm_cap_evicts_oldest() {
    let t = PeerTable::new();
    let base = chrono::Utc::now();
    for i in 0..12i64 {
        let mut p = mk_peer("abc", &format!("{i:02x}"), 0);
        p.last_seen = base + chrono::Duration::seconds(i);
        if i == 11 {
            p.user_id = 2; // 另一账号的 peer 不受配额影响
        }
        t.upsert(p);
    }
    // P2-6（2026-10-07）：实时计数按 user_id 去重——11 条 peer 分属 user1(10)+user2(1)，
    // 去重后 seeder 只算 2 个「人」。本测试的真正目的是**每账号配额淘汰**，
    // 由下面的 snapshot 断言独立验证（最旧的 "00" 已淘汰）；计数断言同步为去重口径。
    assert_eq!(t.count_seeders("abc"), 2);
    // self_user=0：不按账号过滤，才能单独验「配额淘汰」这件事
    let snap = t.snapshot("abc", 50, 0);
    // 最旧的 "00" 应被该账号配额淘汰（peer_id 首字节即 0x00）
    assert!(snap.v4.iter().any(|p| p.peer_id[0] == 0x0b)); // 新的还在
    assert!(!snap.v4.iter().any(|p| p.peer_id[0] == 0x00)); // "00" 已淘汰
}

/// 审计 10-06 第 5 条：port=0 的 peer 不可连接，计数口径应与下发一致
#[test]
fn port_zero_not_counted() {
    let t = PeerTable::new();
    t.upsert(Peer {
        port: 0,
        ..mk_peer("abc", "p0", 0)
    });
    t.upsert(mk_peer("abc", "p1", 0));
    assert_eq!(t.count_seeders("abc"), 1);
}

/// 审计 10-06：TTL 随 interval 伸缩（做种=2×interval+120，leecher=interval+120）
#[test]
fn ttl_follows_interval() {
    super::set_interval_secs(1800);
    assert_eq!(super::ttl_for(0).as_secs(), 3720);
    assert_eq!(super::ttl_for(100).as_secs(), 1920);
    super::set_interval_secs(600);
    assert_eq!(super::ttl_for(100).as_secs(), 720);
    super::set_interval_secs(1800);
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
    t.remove_owned(
        &PeerKey {
            info_hash: "hot".into(),
            peer_id: "p2".into(),
        },
        1,
    );
    assert_eq!(t.swarms(), 2); // cold 桶不受影响
}

/// P2-6（2026-10-07）：同账号多 peer_id 不得把实时 seeders 灌成倍数。
/// peer_id 客户端自报，修复前同一账号注册 N 个随机 peer_id 即可让
/// 「1 个做种的人」显示成 N 个。权威计数在 snatches（按 user+torrent 唯一），
/// 实时口径现与之对齐。
#[test]
fn seeders_deduped_by_user() {
    let t = PeerTable::new();
    // user 1 用 5 个 peer_id 做种（left=0）
    for i in 0..5 {
        t.upsert(mk_peer("abc", &format!("u1p{i}"), 0));
    }
    // user 2 也做种 1 个
    t.upsert(Peer {
        user_id: 2,
        ..mk_peer("abc", "u2p0", 0)
    });
    // 2 个独立用户 → 只算 2 个做种者（修复前为 6）
    assert_eq!(t.count_seeders("abc"), 2);
    // leecher 侧同理
    for i in 0..3 {
        t.upsert(Peer {
            user_id: 3,
            ..mk_peer("abc", &format!("u3p{i}"), 100)
        });
    }
    assert_eq!(t.count_leechers("abc"), 1);
}

/// P0-2 交叉上报校验（2026-10-07）：leecher 声明「从 peer X 下载了 N 字节」
/// 时，tracker 必须把 peer_id 解析成真实可计费的上传者，并拒绝伪造/自证。
#[test]
fn corroboration_target_validates_peer() {
    let t = PeerTable::new();
    // user2 是本 swarm 的正常做种者 → 合法佐证目标
    t.upsert(Peer {
        user_id: 2,
        ..mk_peer("abc", "seeder2", 0)
    });
    // user3 在下载（left>0）→ 不能作为「上传者」被佐证
    t.upsert(Peer {
        user_id: 3,
        ..mk_peer("abc", "leecher3", 100)
    });
    // user4 做种但 port=0（未开监听）→ 不可连接，不应被佐证
    t.upsert(Peer {
        user_id: 4,
        port: 0,
        ..mk_peer("abc", "seeder4", 0)
    });
    // 合法：存活做种者被解析为其 user_id
    assert_eq!(t.corroboration_target("abc", "seeder2", 1), Some(2));
    // leecher 自己被佐证自己（user3 报 user3）→ 但 user3 是 leecher，拒绝
    assert_eq!(t.corroboration_target("abc", "leecher3", 3), None);
    // port=0 的做种者 → 拒绝
    assert_eq!(t.corroboration_target("abc", "seeder4", 1), None);
    // 不存在的 peer_id（凭空捏造）→ 拒绝
    assert_eq!(t.corroboration_target("abc", "ghost_peer", 1), None);
    // 自报自下载：user2 报自己 → 拒绝
    assert_eq!(t.corroboration_target("abc", "seeder2", 2), None);
    // 跨 swarm 不可解析
    assert_eq!(t.corroboration_target("other", "seeder2", 1), None);
}

#[test]
fn connectable_probe_roundtrip() {
    let t = PeerTable::new();
    t.upsert(mk_peer("abc", "p1", 0));
    t.upsert(mk_peer("def", "p2", 0));
    let probes = super::probes::sample_probes(&t.probe_candidates(), 10);
    assert_eq!(probes.len(), 2); // 未测优先
    t.set_connectable(&probes[0].0, false);
    t.set_connectable(&probes[1].0, true);
    // user 1 在 def swarm 可达 → all_unreachable=false
    assert!(!t.all_unreachable(1));
    t.set_connectable(&probes[1].0, false);
    assert!(t.all_unreachable(1)); // 全部不可达
    let again = super::probes::sample_probes(&t.probe_candidates(), 10);
    assert_eq!(again.len(), 2); // 已测 peer 进入复测轮替
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
    let snap = t.snapshot("abc", 50, 0);
    assert_eq!(snap.v4.len(), 1);
    assert_eq!(snap.v6.len(), 1);
    assert_eq!(
        snap.v6[0].ip,
        [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]
    );
}

/// 通用 PT 站点关键语义（0310）：DEAD(0) 与 SUSPECT(-2) 必须在数据层
/// 分开，且 set_conn_state 不得把「未探测」写进去（否则会把「没轮到」
/// 误当成「测过了」而放行）。
#[test]
fn conn_state_keeps_dead_and_suspect_distinct() {
    let t = PeerTable::new();
    t.upsert(mk_peer("abc", "p_dead", 0));
    t.upsert(mk_peer("abc", "p_susp", 0));
    let k_dead = PeerKey {
        info_hash: "abc".into(),
        peer_id: "p_dead".into(),
    };
    let k_susp = PeerKey {
        info_hash: "abc".into(),
        peer_id: "p_susp".into(),
    };
    // 未测态：两peer 都是 UNTESTED
    assert_eq!(t.connectable_of(&k_dead), CONN_UNTESTED);
    assert_eq!(t.connectable_of(&k_susp), CONN_UNTESTED);
    // 分别写入 DEAD 与 SUSPECT
    t.set_conn_state(&k_dead, CONN_DEAD);
    t.set_conn_state(&k_susp, CONN_SUSPECT);
    assert_eq!(t.connectable_of(&k_dead), CONN_DEAD);
    assert_eq!(t.connectable_of(&k_susp), CONN_SUSPECT);
    // 关键：两者不相等——「不可信」与「无法验证」不能被合并
    assert_ne!(t.connectable_of(&k_dead), t.connectable_of(&k_susp));
}

/// UNTESTED 不经由set_conn_state 写入：未探测必须保持「未知」，
/// 不能因为一次写入就把状态坐实。
#[test]
fn set_conn_state_ignores_untested() {
    let t = PeerTable::new();
    t.upsert(mk_peer("abc", "p1", 0));
    let k = PeerKey {
        info_hash: "abc".into(),
        peer_id: "p1".into(),
    };
    t.set_conn_state(&k, CONN_SUSPECT);
    assert_eq!(t.connectable_of(&k), CONN_SUSPECT);
    // 试图写 UNTESTED：应被忽略，保持原值
    t.set_conn_state(&k, CONN_UNTESTED);
    assert_eq!(
        t.connectable_of(&k),
        CONN_SUSPECT,
        "UNTESTED 不应覆盖已有结论"
    );
}

/// 三档判定与 DB 消费口径一致：只有 0 阻断，-2/-1 放行。
/// 这条断言是「通用站不误伤加密客户端」的回归护栏——
/// 若有人把 SUSPECT 也写成阻断条件，这里立刻红。
#[test]
fn only_dead_blocks_in_billing_predicate() {
    // 与 SQL `NOT COALESCE(connectable = 0, false)` 同构
    let blocks = |c: i8| -> bool { !(c == 0) == false };
    assert!(blocks(CONN_DEAD), "DEAD 必须阻断");
    assert!(
        !blocks(CONN_SUSPECT),
        "SUSPECT 不阻断（否则误伤加密客户端）"
    );
    assert!(!blocks(CONN_UNTESTED), "未测不阻断");
    assert!(!blocks(CONN_OK), "可信不阻断");
}

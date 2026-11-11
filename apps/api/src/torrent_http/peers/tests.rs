//! peers 面板单测（外置，主文件行数门禁）。
//! 重点钉住：快照双形态兼容、整份快照不被一行拖垮、身份列契约。

use super::model::*;

/// 客户端指纹只取可打印 ASCII，不可打印补点（防注入进 HTML）
#[test]
fn client_fingerprint_is_printable() {
    // "-qB4480-" 的 hex（qBittorrent 4.4.80）
    let hex = "2d7142343438302d";
    assert_eq!(client_of(hex), "-qB4480-");
    // 含不可打印字节（\u{1}）时补 '.'
    assert_eq!(client_of("01414243"), ".ABC");
    // 奇数长度/非法 hex 不 panic，能解多少算多少
    assert_eq!(client_of("2d7"), "-");
    assert_eq!(client_of(""), "");
}

/// 非 staff 必须脱敏：v4 掩末段、v6 保留前 4 组；staff 看完整
#[test]
fn mask_ip_hides_host_part() {
    assert_eq!(mask_ip("1.2.3.4", false), "1.2.3.*");
    assert_eq!(mask_ip("1.2.3.4", true), "1.2.3.4");
    assert_eq!(
        mask_ip("2001:db8:85a3:8d3:1319:8a2e:370:7348", false),
        "2001:db8:85a3:8d3::*"
    );
    assert_eq!(mask_ip("::1", false), "::1::*");
    // 非法输入不 panic
    assert_eq!(mask_ip("notanip", false), "notanip");
}

/// 进度：做种恒 100；下载中按比例；零总量不除零
#[test]
fn progress_edges() {
    assert_eq!(progress_of(true, 0, 0), 100.0);
    assert_eq!(progress_of(false, 0, 0), 0.0);
    assert_eq!(progress_of(false, 50, 50), 50.0);
    assert_eq!(progress_of(false, 1, 2), 33.3);
}

/// 回归（2026-11-11）：BEP 3/7 之后快照写 v4/v6，旧 SnapPeer
/// 只认顶层 ip/port ⇒ 整份快照解析失败、在线列表恒空。
#[test]
fn snap_peer_accepts_both_endpoint_shapes() {
    let newest = r#"{
            "key":{"info_hash":"aa","peer_id":"bb"},
            "v4":{"ip":"1.2.3.4","port":6881},
            "v6":null,
            "uploaded":0,"downloaded":0,"left":0,
            "last_seen":"2026-11-11T07:47:03.699845905Z",
            "user_id":7,"connectable":-1
        }"#;
    let p: SnapPeer =
        serde_json::from_str(newest).expect("v4/v6 形态必须可解析");
    assert_eq!(p.endpoint(), ("1.2.3.4", 6881));

    // 纯 v6 用户（BEP 7）：v4 为 null 也不能丢
    let v6only = r#"{
            "key":{"info_hash":"aa","peer_id":"bb"},
            "v4":null,
            "v6":{"ip":"2001:db8::1","port":7000},
            "uploaded":0,"downloaded":0,"left":0,
            "last_seen":"2026-11-11T07:47:03.699845905Z",
            "user_id":7,"connectable":-1
        }"#;
    let p: SnapPeer = serde_json::from_str(v6only).unwrap();
    assert_eq!(p.endpoint(), ("2001:db8::1", 7000));

    // 升级前写出的旧行仍要能读（快照 TTL 1800s 内是混形态）
    let legacy = r#"{
            "key":{"info_hash":"aa","peer_id":"bb"},
            "ip":"5.6.7.8","port":6999,
            "uploaded":0,"downloaded":0,"left":100,
            "last_seen":"2026-11-11T07:47:03.699845905Z",
            "user_id":8,"connectable":0
        }"#;
    let p: SnapPeer = serde_json::from_str(legacy).unwrap();
    assert_eq!(p.endpoint(), ("5.6.7.8", 6999));
}

/// 回归：整份 `Vec<(hash, peers)>` 快照不能被新形态行拖垮。
#[test]
fn snapshot_with_v4_shape_survives_full_parse() {
    let raw = r#"[[ "aa", [{
            "key":{"info_hash":"aa","peer_id":"bb"},
            "v4":{"ip":"1.2.3.4","port":6881},"v6":null,
            "uploaded":0,"downloaded":0,"left":0,
            "last_seen":"2026-11-11T07:47:03.699845905Z",
            "user_id":7,"connectable":-1
        }]]]"#;
    type Snap = Vec<(String, Vec<SnapPeer>)>;
    let snap: Snap =
        serde_json::from_str(raw).expect("整份快照必须可解析");
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].1.len(), 1);
    assert_eq!(snap[0].1[0].endpoint(), ("1.2.3.4", 6881));
}

/// 用户名回落：查不到账号时不留空、也不冒充「隐身用户」
#[test]
fn display_name_falls_back_to_id() {
    let names = std::collections::HashMap::from([
        (7_i64, "alice".to_string()),
        (8_i64, "bob".to_string()),
    ]);
    assert_eq!(display_name(&names, 7), "alice");
    assert_eq!(display_name(&names, 99), "#99");
}

/// 契约钉桩：「谁在做种」要求每行都带可点的身份，前端才画得出用户列
#[test]
fn peer_item_exposes_identity() {
    let item = PeerItem {
        user_id: 7,
        username: "alice".into(),
        peer_id: "2d7142343438302d".into(),
        client: "-qB4480-".into(),
        ip: "1.2.3.*".into(),
        port: 6881,
        seeder: true,
        progress: 100.0,
        uploaded: 0,
        downloaded: 0,
        last_seen_secs: 3,
        connectable: -1,
        is_self: false,
    };
    let v = serde_json::to_value(&item).unwrap();
    assert_eq!(v["user_id"], 7);
    assert_eq!(v["username"], "alice");
    assert_eq!(v["seeder"], true);
}

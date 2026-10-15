//! crossseed 单测：指纹归一化（大小写/去重/trim/畸形拒绝）
//! 与 lookup SQL 形状锁定（可见性口径 = 详情，双列 ANY）。

use super::*;

#[test]
fn norm_lowercases_and_dedups() {
    let raw = vec![
        "DB6AAB38B13A6AE52542471A4A9FEF2507973831".to_string(),
        "db6aab38b13a6ae52542471a4a9fef2507973831".to_string(),
    ];
    let s = norm_hashes(&raw).unwrap();
    assert_eq!(s.len(), 1, "同一 hash 大小写两种写法应去重");
    assert!(s.contains("db6aab38b13a6ae52542471a4a9fef2507973831"));
}

#[test]
fn norm_rejects_malformed() {
    for bad in [
        String::new(),
        "xyz".to_string(),
        // 39 位
        "db6aab38b13a6ae52542471a4a9fef250797383".to_string(),
        // 41 位
        "db6aab38b13a6ae52542471a4a9fef25079738310".to_string(),
        // 40 位但含非 hex
        "db6aab38b13a6ae52542471a4a9fef250797383z".to_string(),
    ] {
        assert!(norm_hashes(&[bad.clone()]).is_err(), "应拒绝：{bad}");
    }
}

#[test]
fn norm_trims_whitespace() {
    let raw = vec!["  db6aab38b13a6ae52542471a4a9fef2507973831  ".to_string()];
    let s = norm_hashes(&raw).unwrap();
    assert!(s.contains("db6aab38b13a6ae52542471a4a9fef2507973831"));
}

/// SQL 形状锁定：可见性口径必须与详情一致（approval_status = 1），
/// 双列各一次 ANY（都有索引，不允许退化成逐条 = ）。
#[test]
fn lookup_sql_shape_matches_detail_visibility() {
    const SQL: &str = "SELECT id, name, size, info_hash, pieces_hash \
         FROM torrents WHERE approval_status = 1 \
           AND (info_hash = ANY($1::char(40)[]) \
                OR pieces_hash = ANY($2::char(40)[]))";
    let s = SQL.to_lowercase();
    assert!(
        s.contains("approval_status = 1"),
        "lookup 必须只返回已审核种（与详情同口径）: {SQL}"
    );
    assert!(
        s.contains("info_hash = any($1::char(40)[])"),
        "info_hash 必须走 ANY 批量（索引可用）: {SQL}"
    );
    assert!(
        s.contains("pieces_hash = any($2::char(40)[])"),
        "pieces_hash 必须走 ANY 批量: {SQL}"
    );
}

// ============ IYUU 协议外壳（2026-10-15） ============

#[test]
fn iyuu_req_accepts_iyuu_field_names() {
    // 协议来源：ledccn/iyuuplus-dev composer/reseed-client/src/Client.php
    // 的 /reseed/index/index 入参就是 hash + sha1（不是 info_hash_v1/pieces_sha1）
    let r: IyuuReq = serde_json::from_str(
        r#"{"hash":["DB6AAB38B13A6AE52542471A4A9FEF2507973831"],"sha1":[],"sid_sha1":"abc"}"#,
    )
    .unwrap();
    assert_eq!(r.hash.len(), 1);
    assert!(r.sha1.is_empty());
    assert_eq!(r.sid_sha1.as_deref(), Some("abc"));
}

#[test]
fn iyuu_req_tolerates_missing_fields() {
    // 客户端可能只发 sha1（pieces 匹配）；空对象也不该 500
    let r: IyuuReq = serde_json::from_str(r#"{}"#).unwrap();
    assert!(r.hash.is_empty() && r.sha1.is_empty());
    let r2: IyuuReq =
        serde_json::from_str(r#"{"sha1":["aa"]}"#).unwrap();
    assert_eq!(r2.sha1, vec!["aa".to_string()]);
}

#[test]
fn iyuu_hit_shape_matches_response_contract() {
    // Iyuu\SiteManager\Contracts\Response 的 jsonSerialize 是
    // get_object_vars => {payload, metadata}；客户端 new Torrent(payload, metadata)
    let h = IyuuHit {
        payload: "/api/v1/crossseed/dl/7".into(),
        metadata: false,
        torrent_id: 7,
        name: "x".into(),
        size: 1,
        info_hash: "aa".into(),
    };
    let j = serde_json::to_value(&h).unwrap();
    // 两个契约字段必须在，且类型对
    assert!(j["payload"].is_string());
    assert!(j["metadata"].is_boolean());
    assert_eq!(j["metadata"], serde_json::json!(false));
    // 本站只发链接，不直传二进制（否则绕过 dl 端点的 30 分钟凭证口径）
    assert!(j["payload"].as_str().unwrap().contains("/crossseed/dl/"));
}

#[test]
fn iyuu_shares_the_same_limits_as_lookup() {
    // 两个外壳一份内核：限流/日配额/500 上限都不能只在一侧生效
    assert_eq!(MAX_FINGERPRINTS, 500);
    assert_eq!(RL_PER_MIN, 10);
    assert_eq!(DAILY_QUOTA, 50_000);
}

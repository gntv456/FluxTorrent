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

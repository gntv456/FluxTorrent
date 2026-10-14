//! groups.rs / expected.rs 的单元测试（0354 乐观并发）。
//! 从 settings_http/groups.rs 按域拆出（300 行门禁）。

use super::expected::{check_expected, merge_conflict};
use crate::errors::DomainError;
use std::collections::HashMap;

fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

/// 灰度兼容：expected 缺省（空表）/ 键不在表里 = 不校验直接放行
#[test]
fn expected_absent_skips_check() {
    let empty: HashMap<String, String> = HashMap::new();
    assert!(
        check_expected(&empty, "site_name", ts("2026-01-01T00:00:00Z")).is_ok()
    );
    let mut m = HashMap::new();
    m.insert("other_key".to_string(), "2026-01-01T00:00:00Z".into());
    assert!(check_expected(&m, "site_name", ts("2026-01-01T00:00:00Z")).is_ok());
}

/// 并发模拟（单键口径）：expected 带旧时间戳（他人已先写、updated_at
/// 已前移）→ VersionConflict；带当前值 → 放行
#[test]
fn expected_stale_stamp_conflicts() {
    let mut m = HashMap::new();
    m.insert("site_name".to_string(), "2026-01-01T00:00:00+00:00".into());
    // 库中现值已是 01-02（他人先保存过）→ 旧戳必须拒
    let err = check_expected(&m, "site_name", ts("2026-01-02T00:00:00Z"))
        .unwrap_err();
    match err {
        DomainError::VersionConflict(keys) => {
            assert_eq!(keys, vec!["site_name".to_string()])
        }
        other => panic!("应为 VersionConflict，得到 {other:?}"),
    }
    // 时间戳一致 → 放行（正常保存路径）
    assert!(check_expected(&m, "site_name", ts("2026-01-01T00:00:00Z")).is_ok());
}

/// 非法时间串按冲突处理（客户端口径对不上时不静默放行）
#[test]
fn expected_garbage_stamp_conflicts() {
    let mut m = HashMap::new();
    m.insert("site_name".to_string(), "not-a-timestamp".into());
    assert!(
        check_expected(&m, "site_name", ts("2026-01-02T00:00:00Z")).is_err()
    );
}

/// 冲突键合并：重复键去重，非冲突错误忽略（调用点只喂 VersionConflict）
#[test]
fn merge_conflict_dedups() {
    let mut acc = vec!["a".to_string()];
    merge_conflict(
        &mut acc,
        DomainError::VersionConflict(vec!["a".into(), "b".into()]),
    );
    assert_eq!(acc, vec!["a".to_string(), "b".to_string()]);
}

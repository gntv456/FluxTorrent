//! site_settings 乐观并发（0354）：expected updated_at 校验与冲突合并。
//! groups.rs（分组批量保存）与 admin_http/site_put.rs（旧版单项写）共用；
//! 单元测试在 groups_tests.rs。

use std::collections::HashMap;

use crate::errors::DomainError;

/// 乐观并发校验：客户端带来的 expected updated_at 与库中现值逐键比对，
/// 任一不一致 → VersionConflict（5003/409，data 带冲突键列表）。
/// 灰度兼容三原则：
/// 1. expected 缺省（None / 空表）= 完全不校验，老客户端行为不变；
/// 2. 键不在 expected 里 = 该键不校验（只验客户端明确带回戳的键）；
/// 3. 非法时间串（解析失败）按冲突处理——客户端拿到的是别的口径的
///    时间，静默放行会在灰度期掩盖真并发。
pub(crate) fn check_expected(
    expected: &HashMap<String, String>,
    name: &str,
    current: chrono::DateTime<chrono::Utc>,
) -> Result<(), DomainError> {
    let Some(got) = expected.get(name) else {
        return Ok(()); // 键未携带版本戳：不校验
    };
    match chrono::DateTime::parse_from_rfc3339(got) {
        Ok(t) if t.with_timezone(&chrono::Utc) == current => Ok(()),
        _ => Err(DomainError::VersionConflict(vec![name.to_string()])),
    }
}

/// 合并冲突键：单键校验返回的 VersionConflict 并入已收集列表
/// （整批只报一次 409，data 里列出全部冲突键；重复键去重）
pub(crate) fn merge_conflict(
    acc: &mut Vec<String>,
    e: DomainError,
) {
    if let DomainError::VersionConflict(keys) = e {
        for k in keys {
            if !acc.iter().any(|x| x == &k) {
                acc.push(k);
            }
        }
    }
}

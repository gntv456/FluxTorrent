//! 站点类型包共享类型与校验。
//! 从 pack_apply.rs 按域拆出。

use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct ApplyPackBody {
    pub(crate) code: String,
    /// replace = 清空现有分类重建；merge = 保留现有，仅追加新分类
    #[serde(default)]
    pub(crate) mode: Option<String>,
}

/// 维度 kind 合法性（防注入）：小写字母/数字/下划线
pub(crate) fn is_ascii_kind(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

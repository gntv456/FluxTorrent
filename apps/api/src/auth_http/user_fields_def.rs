//! 用户自定义字段的**定义侧**入参与校验（user_fields.rs 超行数门禁后拆出）。
//!
//! 放这里的是「一条字段定义长什么样、什么算合法」；读写数据库的 CRUD
//! 留在 `user_fields.rs`。类型语义本身仍以 `crate::fields` 为唯一真值源，
//! 本文件只负责把后台提交的东西挡在门口。

use serde::Deserialize;

use crate::errors::{DomainError, DomainResult};

/// 字段 key：小写字母/数字/下划线，≤40。注册表与 user_field_values 都按它关联，
/// 放宽一次就得永远兼容两套脏键。
pub(crate) fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 40
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub(crate) fn valid_field_type(t: &str) -> bool {
    crate::fields::valid_field_type(t)
}

/// 空串按「不挂模块」处理（后台清空选择框提交的就是空串）
pub(crate) fn norm_module_key(k: &Option<String>) -> Option<&str> {
    k.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn default_vis() -> String {
    "public".into()
}
fn default_sort() -> i32 {
    100
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
pub(crate) struct UserFieldDefBody {
    pub label: String,
    pub r#type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_vis")]
    pub visibility: String,
    #[serde(default)]
    pub show_on_register: bool,
    #[serde(default)]
    pub options: serde_json::Value,
    #[serde(default = "default_sort")]
    pub sort: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 挂到某模块键（空/缺省 = 不挂，恒可见）
    #[serde(default)]
    pub module_key: Option<String>,
}

pub(crate) fn validate_def(body: &UserFieldDefBody) -> DomainResult<()> {
    if body.label.trim().is_empty() || body.label.len() > 50 {
        return Err(DomainError::Validation("字段名需 1-50 字符".into()));
    }
    if !valid_field_type(&body.r#type) {
        return Err(DomainError::Validation(
            "type 需为 text/number/select/multiselect/date/bool".into(),
        ));
    }
    if !["public", "private"].contains(&body.visibility.as_str()) {
        return Err(DomainError::Validation(
            "visibility 需为 public/private".into(),
        ));
    }
    if matches!(body.r#type.as_str(), "select" | "multiselect") {
        let arr = body.options.as_array().ok_or_else(|| {
            DomainError::Validation("select 类型需提供 options 数组".into())
        })?;
        if arr.is_empty() || arr.len() > 50 {
            return Err(DomainError::Validation("options 需 1-50 项".into()));
        }
        for o in arr {
            let val = o
                .get("value")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    DomainError::Validation(
                        "options 项需含 value 字符串".into(),
                    )
                })?;
            if val.is_empty() || val.len() > 100 {
                return Err(DomainError::Validation(
                    "option value 需 1-100 字符".into(),
                ));
            }
        }
    }
    Ok(())
}

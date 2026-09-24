//! 查询参数的「宽松反序列化」助手（2026-09-23 从 query.rs 按域拆出，
//! 守 300 行门禁）。只做「把表单/第三方客户端的宽容写法转成 Rust 类型」，
//! 无副作用、无状态。

use serde::Deserialize;

/// 宽松布尔解析（0093）：查询串里的 `1/0/true/false/yes/no` 均接受。
/// 此前 `include_dead=1`（旧站 1/0 口径、第三方客户端常用）会让整个 Query 反序列化失败 → 400。
pub(super) fn de_bool_lenient<'de, D>(d: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<String>::deserialize(d)?;
    Ok(v.map(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    }))
}

/// 宽容的数字反序列化（005 修复）：表单里「全部」这类选项会提交 `alive=&tag_id=` 空串，
/// 而 `Option<i32>` 直接吃空串会解析失败 → 整个 Query 反序列化报错 → 400。
/// 这里统一把空/空白视为 None，非法值才算错。
pub(super) fn de_opt_num_lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let v = Option::<String>::deserialize(d)?;
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => s.parse::<T>().map(Some).map_err(serde::de::Error::custom),
    }
}

/// tag_ids 的宽松反序列化：seq → 原样；字符串 → 按逗号拆（与 category_ids 同套路）
pub(super) fn de_tag_ids_lenient<'de, D>(
    d: D,
) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<String>),
        One(String),
    }
    let v = serde_json::Value::deserialize(d)?;
    let pick = serde_json::from_value::<OneOrMany>(v)
        .map_err(serde::de::Error::custom)?;
    Ok(match pick {
        OneOrMany::Many(v) => v,
        OneOrMany::One(s) => {
            s.split(',').map(|x| x.trim().to_string()).collect()
        }
    })
}

/// category_ids/category_id 的宽松反序列化：seq → 原样；字符串 → 按逗号拆。
/// （此前用 alias 兼容单值键名，但 serde 对 Vec 字段的裸字符串直接报错 → 前端筛选 400）
pub(super) fn de_category_ids_lenient<'de, D>(
    d: D,
) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<String>),
        One(String),
    }
    // actix-web Query 的 serde_qs 形状：字段值是「单值或 seq」的直接载荷
    let v = serde_json::Value::deserialize(d)?;
    let pick = serde_json::from_value::<OneOrMany>(v)
        .map_err(serde::de::Error::custom)?;
    Ok(match pick {
        OneOrMany::Many(v) => v,
        OneOrMany::One(s) => {
            s.split(',').map(|x| x.trim().to_string()).collect()
        }
    })
}

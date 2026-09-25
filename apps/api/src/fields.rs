//! 字段类型系统（B 批承重墙·B2）。
//!
//! 一套类型语义，两处消费：
//!   · 用户侧自定义字段（0186_user_fields：`user_field_defs` / `user_field_values`）
//!   · 内容侧自定义维度（0195_section_field_types：`section_kinds` / `torrent_sections`）
//!
//! 六类型与 `user_field_defs.type`、`section_kinds.field_type` 的 CHECK 约束**逐字对齐**：
//! `text` / `number` / `select` / `multiselect` / `date` / `bool`。
//!
//! 为什么统一到这里：四审 L3 判定内容侧「只能表达单选枚举」，而用户侧早已有完整
//! 六类型系统——内容侧缺的不是能力，是「把已有能力铺过去」。共用同一校验器后，
//! 两侧的类型语义不会各自漂移。
//!
//! **本模块不改动用户侧现有语义**：`auth_http::user_fields::validate_value` 保留为
//! 薄转发，register.rs 等既有调用点零改动。

/// 六类型取值集合（与两处 DB CHECK 约束逐字对齐）。
pub const FIELD_TYPES: [&str; 6] =
    ["text", "number", "select", "multiselect", "date", "bool"];

/// 类型是否合法。
pub fn valid_field_type(t: &str) -> bool {
    FIELD_TYPES.contains(&t)
}

/// 从 options（`[{value,label}]`）提取允许值集合。
/// 用户侧来自 `user_field_defs.options`；内容侧来自 `section_dict` 的名字列表。
fn allowed_values(options: &serde_json::Value) -> Vec<String> {
    options
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    o.get("value")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 校验值形状与字段类型匹配；`select`/`multiselect` 额外校验值在 options 内。
///
/// 值格式（与 0186 的用户侧一致，也是内容侧自由值的存储形态）：
/// - `text`        → 字符串（≤500 字符）
/// - `number`      → 数字
/// - `date`        → `YYYY-MM-DD` 字符串
/// - `bool`        → 布尔
/// - `select`      → 字符串，且 ∈ options
/// - `multiselect` → 字符串数组（≤20 项），每项 ∈ options
pub fn validate_value(
    field_type: &str,
    options: &serde_json::Value,
    v: &serde_json::Value,
) -> Result<(), String> {
    let allowed = allowed_values(options);
    match field_type {
        "text" => {
            v.as_str()
                .filter(|s| s.len() <= 500)
                .map(|_| ())
                .ok_or("text 字段需为 ≤500 字符字符串")?;
        }
        "number" => {
            v.as_f64().map(|_| ()).ok_or("number 字段需为数字")?;
        }
        "date" => {
            let s = v.as_str().ok_or("date 字段需为 YYYY-MM-DD 字符串")?;
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|_| ())
                .map_err(|_| "date 字段需为 YYYY-MM-DD".to_string())?;
        }
        "bool" => {
            v.as_bool().map(|_| ()).ok_or("bool 字段需为 true/false")?;
        }
        "select" => {
            let s = v.as_str().ok_or("select 字段需为选项值字符串")?;
            if !allowed.contains(&s.to_string()) {
                return Err("值不在字段选项集内".into());
            }
        }
        "multiselect" => {
            let arr = v.as_array().ok_or("multiselect 字段需为选项值数组")?;
            if arr.len() > 20 {
                return Err("multiselect 最多 20 项".into());
            }
            for item in arr {
                let s =
                    item.as_str().ok_or("multiselect 数组元素需为字符串")?;
                if !allowed.contains(&s.to_string()) {
                    return Err("值不在字段选项集内".into());
                }
            }
        }
        _ => return Err("未知字段类型".into()),
    }
    Ok(())
}

/// 必填语义的统一判定：空值 = null / 空串 / 空数组 / 空对象。
///
/// 与用户侧 `my_fields_put` 的既有语义一致（null 表示清除），内容侧据此挡必填维度缺值。
pub fn is_empty_value(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.trim().is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> serde_json::Value {
        serde_json::json!([
            {"value": "a", "label": "A"},
            {"value": "b", "label": "B"}
        ])
    }

    #[test]
    fn six_types_recognised() {
        for t in FIELD_TYPES {
            assert!(valid_field_type(t), "{t}");
        }
        assert!(!valid_field_type("textarea"));
        assert!(!valid_field_type(""));
    }

    #[test]
    fn text_number_date_bool_shapes() {
        let o = serde_json::json!([]);
        let v = |x: serde_json::Value| validate_value("text", &o, &x);
        assert!(v(serde_json::json!("hello")).is_ok());
        assert!(v(serde_json::json!(5)).is_err());
        let n = |x: serde_json::Value| validate_value("number", &o, &x);
        assert!(n(serde_json::json!(320)).is_ok());
        assert!(n(serde_json::json!("320")).is_err());
        let d = |x: serde_json::Value| validate_value("date", &o, &x);
        assert!(d(serde_json::json!("2024-05-01")).is_ok());
        assert!(d(serde_json::json!("2024/05/01")).is_err());
        let b = |x: serde_json::Value| validate_value("bool", &o, &x);
        assert!(b(serde_json::json!(true)).is_ok());
        assert!(b(serde_json::json!("true")).is_err());
    }

    #[test]
    fn select_and_multiselect_must_be_in_options() {
        let s = |x: serde_json::Value| validate_value("select", &opts(), &x);
        assert!(s(serde_json::json!("a")).is_ok());
        assert!(s(serde_json::json!("z")).is_err());
        let m =
            |x: serde_json::Value| validate_value("multiselect", &opts(), &x);
        assert!(m(serde_json::json!(["a", "b"])).is_ok());
        assert!(m(serde_json::json!(["a", "z"])).is_err());
        // 上限 20 项
        let big: Vec<String> = (0..21).map(|i| format!("a{i}")).collect();
        let big_with_a = serde_json::json!(big);
        assert!(m(big_with_a).is_err());
    }

    #[test]
    fn text_length_capped() {
        let o = serde_json::json!([]);
        let long = "x".repeat(501);
        assert!(validate_value("text", &o, &serde_json::json!(long)).is_err());
        let ok = "x".repeat(500);
        assert!(validate_value("text", &o, &serde_json::json!(ok)).is_ok());
    }

    #[test]
    fn empty_value_semantics() {
        assert!(is_empty_value(&serde_json::Value::Null));
        assert!(is_empty_value(&serde_json::json!("")));
        assert!(is_empty_value(&serde_json::json!("   ")));
        assert!(is_empty_value(&serde_json::json!([])));
        assert!(is_empty_value(&serde_json::json!({})));
        assert!(!is_empty_value(&serde_json::json!("x")));
        assert!(!is_empty_value(&serde_json::json!(0)));
        assert!(!is_empty_value(&serde_json::json!(false)));
    }
}

//! 分类学包的维度行形状（B3，2026-09-25）。
//!
//! `section_kinds` 的六类型元数据（field_type/required/multiple/enabled/
//! icon_key/bg_color）在**导出、快照、导入解析、落库**四处必须同一形状——
//! 任何一处漏列，走一圈包（导出→导入 / 导入→回滚）后维度就静默降级回
//! select，number/date/bool/text 的存量值在筛选与表单侧全部失明。
//! 因此把行结构与 JSON 互转收敛在本文件，四处只调用不复制。

use serde_json::Value;

/// 维度行全字段（与 section_kinds 表列一一对应）
#[derive(sqlx::FromRow)]
pub(crate) struct KindRow {
    pub kind: String,
    pub label: String,
    pub sort: i32,
    pub field_type: String,
    pub required: bool,
    pub multiple: bool,
    pub enabled: bool,
    pub icon_key: Option<String>,
    pub bg_color: Option<String>,
}

/// KindRow → 包 JSON（None 字段落 null：apply 侧按缺省处理）
pub(crate) fn kind_to_json(k: &KindRow) -> Value {
    serde_json::json!({
        "kind": k.kind, "label": k.label, "sort": k.sort,
        "field_type": k.field_type, "required": k.required,
        "multiple": k.multiple, "enabled": k.enabled,
        "icon_key": k.icon_key, "bg_color": k.bg_color,
    })
}

/// 包 JSON → KindRow。旧包/站型包不带元数据字段 → 缺省值与
/// section_kinds 列缺省一致（select / 非必填 / 启用）。
/// field_type 不合法直接 Err——静默回落 select 会让 number 维度的
/// 存量值在筛选/表单侧全部失明（宁拒收不降级）。
pub(crate) fn kind_from_json(
    k: &Value,
    bad: &dyn Fn(&str) -> crate::errors::DomainError,
) -> Result<KindRow, crate::errors::DomainError> {
    let kind = k.get("kind").and_then(Value::as_str).unwrap_or_default();
    let label = k.get("label").and_then(Value::as_str).unwrap_or_default();
    let field_type = k
        .get("field_type")
        .and_then(Value::as_str)
        .unwrap_or("select")
        .trim()
        .to_string();
    if !crate::fields::valid_field_type(&field_type) {
        return Err(bad(&format!(
            "维度 {kind} 的 field_type 非法：{field_type}\
             （可选 text/number/select/multiselect/date/bool）"
        )));
    }
    Ok(KindRow {
        kind: kind.to_string(),
        label: label.to_string(),
        sort: k.get("sort").and_then(Value::as_i64).unwrap_or(999) as i32,
        field_type,
        required: k.get("required").and_then(Value::as_bool).unwrap_or(false),
        multiple: k.get("multiple").and_then(Value::as_bool).unwrap_or(false),
        enabled: k.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        icon_key: k
            .get("icon_key")
            .and_then(Value::as_str)
            .map(str::to_string),
        bg_color: k
            .get("bg_color")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

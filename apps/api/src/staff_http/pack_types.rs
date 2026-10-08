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

/// 站型包 `sections.kinds` 落库（apply_pack_full 调用）。
///
/// 复用分类学包的九列全字段解析（`pack_kinds::kind_from_json`，单一真相源）：
/// 站型包此前只落 (kind,label,sort)，field_type/required/multiple/enabled
/// 全部落回列缺省（select/非必填/单值）⇒ 六类型字段系统对站型静默失效，
/// 站型包配的 text/number/bool 维度一律降级成单选枚举。
///
/// 纪律：
/// - 复用 `is_ascii_kind` 守卫（与旧实现一致，非法 kind 跳过不报错）；
/// - `field_type` **不可改**（存量值按原类型解释，同 admin PUT 纪律），
///   故 ON CONFLICT 只更新展示属性（label/sort/图标/底色），不动类型与开关；
/// - `kind_from_json` 对非法 field_type 直接 Err——站型包 apply 走`continue`
///   跳过该维度（与旧实现「解析不了就跳过」同口径，不因一个坏维度中断整包）。
pub(crate) async fn apply_pack_kinds(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kinds: &[serde_json::Value],
) -> crate::errors::DomainResult<()> {
    use crate::errors::DomainError;
    let bad = |msg: &str| DomainError::Validation(msg.to_string());
    for k in kinds {
        let parsed = crate::settings_http::pack_kinds::kind_from_json(k, &bad);
        let Ok(row) = parsed else { continue };
        if !is_ascii_kind(&row.kind) {
            continue;
        }
        sqlx::query(
            "INSERT INTO section_kinds \
               (kind, label, sort, field_type, required, multiple, \
                enabled, icon_key, bg_color) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (kind) DO UPDATE SET label = EXCLUDED.label, \
                sort = EXCLUDED.sort, icon_key = EXCLUDED.icon_key, \
                bg_color = EXCLUDED.bg_color",
        )
        .bind(&row.kind)
        .bind(&row.label)
        .bind(row.sort)
        .bind(&row.field_type)
        .bind(row.required)
        .bind(row.multiple)
        .bind(row.enabled)
        .bind(row.icon_key.as_deref())
        .bind(row.bg_color.as_deref())
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

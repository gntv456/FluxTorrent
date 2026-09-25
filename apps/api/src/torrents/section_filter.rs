//! 内容侧维度筛选的类型分派（B3，2026-09-25）。从 `section_pred.rs` 拆出。
//! `SectionFilter` = 单个维度条件；`build_section_filters` 归并 query 抓到的
//! `(kind, op, value)`；`typed_pred` 按 field_type 生成命中谓词。
//!
//! ⚠ 谓词是**字符串内插**（列表/计数两侧共用一个返回值，故不走 bind），
//! 文本值必须经 `lit()` 转义。kind 已过 `section_kinds` 白名单。
//! 无法解释的输入一律**不生成谓词**（保守），绝不退化成「全部可见」。

use sqlx::PgPool;

/// 单个维度条件。`values` 为文本原样：枚举 = dict_id 十进制串，
/// bool = "true"/"false"，text = 关键词，number/date = 精确值（区间用 min/max）
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SectionFilter {
    pub kind: String,
    pub field_type: String,
    pub values: Vec<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

impl SectionFilter {
    /// 是否有任何可用筛选意图（全空 = 不参与谓词）
    pub fn is_active(&self) -> bool {
        !self.values.is_empty() || self.min.is_some() || self.max.is_some()
    }
}

/// 把 query 抓到的 `(kind, op, value)` 归并成按维度聚合的筛选条件。
/// `op`：`eq`（等值/多选，可重复）/ `min` / `max`。未知 kind 或查不到
/// field_type 的直接丢弃（不报错——筛选参数宽松是既有口径）。
pub(crate) async fn build_section_filters(
    db: &PgPool,
    raw: Vec<(String, String, String)>,
) -> Vec<SectionFilter> {
    use std::collections::BTreeMap;
    let mut by_kind: BTreeMap<String, SectionFilter> = BTreeMap::new();
    for (kind, op, val) in raw {
        let ft = match crate::admin_p3_http::kind_field_type(db, &kind).await {
            Some(t) => t,
            None => continue,
        };
        let e = by_kind
            .entry(kind.clone())
            .or_insert_with(|| SectionFilter {
                kind: kind.clone(),
                field_type: ft,
                ..Default::default()
            });
        match op.as_str() {
            "min" => e.min = Some(val),
            "max" => e.max = Some(val),
            _ => {
                if !e.values.contains(&val) {
                    e.values.push(val);
                }
            }
        }
    }
    by_kind.into_values().collect()
}

/// SQL 字面量转义。Postgres `standard_conforming_strings=on` 下 `''` 即可；
/// 仍剔除 NUL 并截断 200 字符（筛选关键词没有合理理由更长）。
pub(crate) fn lit(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| *c != '\0').take(200).collect();
    format!("'{}'", cleaned.replace('\'', "''"))
}

/// 按 field_type 生成「该维度命中」的谓词；无法解释时返回 `None`。
pub(crate) fn typed_pred(kind: &str, c: &SectionFilter) -> Option<String> {
    let hit = |extra: &str| {
        Some(format!(
            "t.id IN (SELECT torrent_id FROM torrent_sections \
             WHERE kind = '{kind}' AND {extra})"
        ))
    };
    match c.field_type.as_str() {
        // 枚举：值即 dict_id（多值由外层 OR 拼接）
        "select" | "multiselect" => {
            let ids: Vec<&String> =
                c.values.iter().filter(|v| is_uint(v)).collect();
            if ids.is_empty() {
                return None;
            }
            let arr = format!("ARRAY[{}]::bigint[]", join(ids));
            hit(&format!("dict_id = ANY({arr})"))
        }
        // 三态（true / false）；无有效取值 ⇒ 无谓词
        "bool" => {
            let preds = bool_preds(&c.values);
            if preds.is_empty() {
                return None;
            }
            hit(&preds.join(" OR "))
        }
        // 数字：区间（jsonb 数字）。非数字输入忽略，避免非法 cast 变 500。
        "number" => {
            let bounds = range_bounds(c, &is_num, RangeMode::Num);
            if bounds.is_empty() {
                return None;
            }
            // 必须限定 jsonb 本身是数字，否则字符串值 cast 会 500
            let guard = "value IS NOT NULL AND jsonb_typeof(value) = 'number'";
            hit(&format!("{guard} AND ({})", bounds.join(" AND ")))
        }
        // 日期：ISO 串字典序即时间序
        "date" => {
            let bounds = range_bounds(c, &is_date, RangeMode::Date);
            if bounds.is_empty() {
                return None;
            }
            hit(&format!("value IS NOT NULL AND ({})", bounds.join(" AND ")))
        }
        // 文本：关键词子串（大小写不敏感）
        "text" => {
            let Some(kw) = c.values.first().filter(|v| !v.trim().is_empty())
            else {
                return None;
            };
            let esc = kw
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let pat = lit(&format!("%{esc}%"));
            hit(&format!("value #>> '{{}}' ILIKE {pat} ESCAPE chr(92)"))
        }
        // 未知类型：不生成谓词
        _ => None,
    }
}

/// 布尔维度的命中谓词（去重；无法识别的取值忽略）。
fn bool_preds(values: &[String]) -> Vec<String> {
    const T: &str = "value = 'true'::jsonb";
    const F: &str = "value = 'false'::jsonb";
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let want = match v.as_str() {
            "true" | "1" => T,
            "false" | "0" => F,
            _ => continue,
        };
        if !out.iter().any(|s| s == want) {
            out.push(want.to_string());
        }
    }
    out
}

/// 区间比较的取值方式：Num = jsonb 直接 cast numeric；Date = `value #>> '{}'`
/// 取裸文本（ISO 串字典序即时间序）。
#[derive(Clone, Copy)]
enum RangeMode {
    Num,
    Date,
}

/// 数字/日期区间边界（含端点）。
fn range_bounds(
    c: &SectionFilter,
    ok: &dyn Fn(&str) -> bool,
    mode: RangeMode,
) -> Vec<String> {
    // 左值表达式（不含具体值）；右值字面量：数字裸内插（已过 is_num），日期转义
    let lhs = match mode {
        RangeMode::Num => "(value)::text::numeric".to_string(),
        RangeMode::Date => "value #>> '{}'".to_string(),
    };
    let rhs = |v: &str| match mode {
        RangeMode::Num => v.to_string(),
        RangeMode::Date => lit(v),
    };
    let mut out: Vec<String> = Vec::new();
    if let Some(v) = c.min.as_deref().filter(|v| ok(v)) {
        out.push(format!("{lhs} >= {}", rhs(v)));
    }
    if let Some(v) = c.max.as_deref().filter(|v| ok(v)) {
        out.push(format!("{lhs} <= {}", rhs(v)));
    }
    if let Some(v) = c.values.first().filter(|v| ok(v)) {
        out.push(format!("{lhs} = {}", rhs(v)));
    }
    out
}

fn join(items: Vec<&String>) -> String {
    items
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(",")
}
fn is_uint(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
fn is_num(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}
/// 宽松日期校验：`YYYY-MM-DD` 前缀（也接受完整 ISO 串）
fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::{is_date, is_num, is_uint, typed_pred, SectionFilter};

    fn mk(
        ft: &str,
        values: &[&str],
        min: Option<&str>,
        max: Option<&str>,
    ) -> SectionFilter {
        SectionFilter {
            kind: "k".into(),
            field_type: ft.into(),
            values: values.iter().map(|s| s.to_string()).collect(),
            min: min.map(str::to_string),
            max: max.map(str::to_string),
        }
    }

    #[test]
    fn enum_pred_lists_dict_ids() {
        let f = mk("select", &["5", "8"], None, None);
        let p = typed_pred("k", &f).unwrap();
        assert!(p.contains("dict_id = ANY(ARRAY[5,8]::bigint[])"), "{p}");
    }

    #[test]
    fn enum_pred_drops_non_numeric() {
        // 非数字直接忽略：全非法 ⇒ 无谓词（不是「匹配全部」）
        let f = mk("select", &["abc"], None, None);
        assert!(typed_pred("k", &f).is_none());
    }

    #[test]
    fn bool_pred_two_state() {
        let p = typed_pred("k", &mk("bool", &["true"], None, None)).unwrap();
        assert!(p.contains("value = 'true'::jsonb"), "{p}");
        let f = mk("bool", &["x"], None, None);
        assert!(typed_pred("k", &f).is_none());
    }

    #[test]
    fn number_pred_range() {
        let p = typed_pred("k", &mk("number", &[], Some("10"), Some("20")))
            .unwrap();
        assert!(p.contains(">= 10") && p.contains("<= 20"), "{p}");
        // 非法数字忽略 ⇒ 两个都非法 = 无谓词
        let bad = mk("number", &[], Some("a"), Some("b"));
        assert!(typed_pred("k", &bad).is_none());
    }

    #[test]
    fn date_pred_escapes_and_ranges() {
        let d1 = mk("date", &[], Some("2024-01-01"), Some("2024-12-31"));
        let p = typed_pred("k", &d1).unwrap();
        // 左值只能有一次 `#>> '{}'`，右值是单个字面量（曾把值也拼进左值 → PG 500）
        assert!(p.contains("value #>> '{}' >= '2024-01-01'"), "{p}");
        assert!(p.contains("value #>> '{}' <= '2024-12-31'"), "{p}");
        assert!(!p.contains("'{}' '2"), "左值混入了值: {p}");
        let bad = mk("date", &["2024-13"], None, None);
        assert!(typed_pred("k", &bad).is_none());
    }

    #[test]
    fn number_pred_guards_jsonb_type() {
        let p = typed_pred("k", &mk("number", &[], Some("10"), None)).unwrap();
        // 必须限定 jsonb 是数字，否则字符串值 cast 会 500
        assert!(p.contains("jsonb_typeof(value) = 'number'"), "{p}");
    }

    #[test]
    fn text_pred_escapes_quote() {
        let p = typed_pred("k", &mk("text", &["a'b"], None, None)).unwrap();
        assert!(p.contains("'%a''b%'"), "{p}");
        let f = mk("text", &["  "], None, None);
        assert!(typed_pred("k", &f).is_none());
    }

    #[test]
    fn predicates_use_exists_not_join() {
        // 谓词必须是 `t.id IN (SELECT …)`，避免与主查询 FROM 串味
        let p = typed_pred("k", &mk("select", &["1"], None, None)).unwrap();
        assert!(p.starts_with("t.id IN (SELECT torrent_id"), "{p}");
    }

    #[test]
    fn helpers_sane() {
        assert!(is_uint("123") && !is_uint("") && !is_uint("1a"));
        assert!(is_num("1.5") && !is_num("x"));
        assert!(is_date("2024-05-01") && !is_date("2024-5-1"));
    }
}

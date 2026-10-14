//! 保种区查询参数的白名单（P-287）。
//!
//! 从 `preserve.rs` 拆出来只为守行数门（该文件贴 300 上限），
//! 语义与调用点无关：未填/空串=不筛，未知值=400 且把可用值写在 message 里。

use crate::errors::{DomainError, DomainResult};

pub const SCOPES: [&str; 3] = ["all", "official", "general"];
pub const STATUSES: [&str; 4] = ["current", "active", "grace", "expired"];

/// 口径与种列表的 promo/date/size 一致（P-279 同族）：
/// 表单清空后照样提交空串 ⇒ 空串按「未填」，不能报错。
pub fn norm_choice(
    val: Option<&str>,
    legal: &[&str],
    what: &str,
) -> DomainResult<Option<String>> {
    let Some(raw) = val else {
        return Ok(None);
    };
    let v = raw.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if legal.contains(&v) {
        return Ok(Some(v.to_string()));
    }
    Err(DomainError::Validation(format!(
        "{what}取值无效：{v}（可用 {}）",
        legal.join("/")
    )))
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    #[test]
    fn empty_and_absent_mean_no_filter() {
        assert_eq!(norm_choice(None, &SCOPES, "保种类型").unwrap(), None);
        assert_eq!(norm_choice(Some("  "), &SCOPES, "保种类型").unwrap(), None);
    }

    #[test]
    fn legal_values_pass_through() {
        assert_eq!(
            norm_choice(Some("official"), &SCOPES, "保种类型").unwrap(),
            Some("official".into())
        );
        assert_eq!(
            norm_choice(Some("grace"), &STATUSES, "保种状态").unwrap(),
            Some("grace".into())
        );
    }

    #[test]
    fn unknown_is_error_with_hint() {
        let e = norm_choice(Some("9"), &SCOPES, "保种类型").unwrap_err();
        let m = format!("{e}");
        assert!(m.contains("保种类型取值无效：9"), "{m}");
        assert!(m.contains("all/official/general"), "{m}");
    }
}


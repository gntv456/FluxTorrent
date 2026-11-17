//! Wiki 请求体校验 + 单测（0495 从 wiki.rs 拆出——300 行门禁第三次拆分：
//! 0359 拆 wiki_public，本批 section 字段把 CRUD 顶过线）。
//!
//! 校验规则不变：slug 小写字母/数字/连字符 ≤60；标题按字符 1-100
//! （按字节会让 34 字中文页名被自拒，批五十三 P-200）；正文 ≤200KB。
//! section 是自由串（前台按固定组键归组，未知键落「未分组」卡）。

use serde::Deserialize;

use crate::errors::{DomainError, DomainResult};

/// 与 custom_pages 同口径：小写字母/数字/连字符，非空且 ≤60 字节
pub(crate) fn valid_wiki_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 60
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// 正文上限 200KB（与 custom_pages 同档）
pub(crate) const WIKI_BODY_LIMIT: usize = 200_000;

#[derive(Deserialize)]
pub(crate) struct WikiBody {
    pub(crate) slug: String,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) body: String,
    #[serde(default = "default_true")]
    pub(crate) visible: bool,
    #[serde(default = "default_sort")]
    pub(crate) sort: i32,
    /// 0495 分组目录：start/site/rules/api 等组键，缺省 ''（未分组）
    #[serde(default)]
    pub(crate) section: String,
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

pub(crate) fn validate_wiki_body(b: &WikiBody) -> DomainResult<()> {
    if !valid_wiki_slug(&b.slug) {
        return Err(DomainError::Validation(
            "slug 需为小写字母/数字/连字符，≤60 字符".into(),
        ));
    }
    // 按字符判长：wiki 页名是站长写的中文标题，按字节判会让 34 字的正常
    // 页名被自拒（custom_pages 同族，见批五十三 P-200）
    if b.title.trim().is_empty() || b.title.chars().count() > 100 {
        return Err(DomainError::Validation("标题需 1-100 字符".into()));
    }
    if b.body.len() > WIKI_BODY_LIMIT {
        return Err(DomainError::Validation("正文超过 200KB 上限".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wb(slug: &str, title: &str, body: &str) -> WikiBody {
        WikiBody {
            slug: slug.into(),
            title: title.into(),
            body: body.into(),
            visible: true,
            sort: 100,
            section: String::new(),
        }
    }

    #[test]
    fn slug_rules_match_custom_pages() {
        assert!(valid_wiki_slug("a"));
        assert!(valid_wiki_slug("beginner-guide-2026"));
        assert!(valid_wiki_slug(&"x".repeat(60)));
        for bad in ["", "A", "Upper", "sp ace", "下划线_1", "dot.slug"] {
            assert!(!valid_wiki_slug(bad), "{bad} 不应通过");
        }
        assert!(!valid_wiki_slug(&"x".repeat(61)), "超 60 应拒绝");
    }

    #[test]
    fn body_over_200kb_rejected() {
        let big = "x".repeat(WIKI_BODY_LIMIT + 1);
        assert!(validate_wiki_body(&wb("ok", "t", &big)).is_err());
        assert!(validate_wiki_body(&wb("ok", "t", "")).is_ok());
    }

    #[test]
    fn title_bounds_enforced() {
        assert!(validate_wiki_body(&wb("ok", "", "b")).is_err());
        // 101 字符 > 100（按 chars 计，非字节）
        let long = "标".repeat(101);
        assert!(validate_wiki_body(&wb("ok", &long, "b")).is_err());
        assert!(validate_wiki_body(&wb("ok", "标题", "b")).is_ok());
    }

    #[test]
    fn section_field_accepted() {
        // 0495：section 任意串（前台按固定组键归组，未知键落「未分组」）
        let mut b = wb("ok", "标题", "b");
        b.section = "start".into();
        assert!(validate_wiki_body(&b).is_ok());
    }
}

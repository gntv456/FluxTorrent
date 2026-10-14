//! 列表筛选参数的严格档（批七十五 P-279，从 `query.rs` 的宽容档之上加一层）。
//!
//! `norm_date` / `parse_size` 的既有职责是**别让烂字符串走到 PG 的 `$n::date`
//! 抛错变成 500**（2026-10-10 实测过），它们对无效值一律返回 `None`。
//! 但 `None` 在 SQL 侧是「不加谓词」，于是会员填了 `date_from=2025-13-45`
//! 或 `size_min=abc` 拿到的是**全站**——不是报错，也不是空，而是「假装筛过了」。
//! 本仓对同一形状已有三次收口：`category_id`（非法 ⇒ 400）、`approval`
//! （越界 ⇒ 400，注释原话「答非所问还不报错」）、`promo`（批七十四 ⇒ 400）。
//! 这一层把日期/体积/匿名三态补齐成同一口径：
//! **空值（表单未填）= 不筛；给了值却解析不出来 = 400 站内信封**。

use crate::errors::{DomainError, DomainResult};
use crate::torrent_http::query::{norm_date, parse_size};

/// 日期：空 ⇒ 不筛；`YYYY-MM-DD` 且日历上存在 ⇒ 放行；其余 ⇒ 400。
pub(crate) fn date_or_err(
    s: Option<String>,
) -> DomainResult<Option<String>> {
    if s.as_deref().unwrap_or("").trim().is_empty() {
        return Ok(None);
    }
    let shown = s.clone().unwrap_or_default();
    match norm_date(s) {
        Some(v) => Ok(Some(v)),
        None => Err(DomainError::Validation(format!(
            "日期格式无效：{shown}（要用 YYYY-MM-DD，且日历上得真有这一天）"
        ))),
    }
}

/// 体积：空 ⇒ 不筛；`1024`/`500MB`/`1.5GB`/`2TB`（含 KiB/GiB）⇒ 字节数；
/// 其余（含非正数）⇒ 400。
pub(crate) fn size_or_err(
    s: Option<String>,
) -> DomainResult<Option<i64>> {
    if s.as_deref().unwrap_or("").trim().is_empty() {
        return Ok(None);
    }
    let shown = s.clone().unwrap_or_default();
    match parse_size(s) {
        Some(v) => Ok(Some(v)),
        None => Err(DomainError::Validation(format!(
            "体积无效：{shown}（要正数，可带 KB/MB/GB/TB 单位）"
        ))),
    }
}

/// 匿名发布三态：`0` 全部 / `1` 仅匿名 / `2` 仅具名；空 ⇒ 不筛；
/// 其余 ⇒ 400（旧行为是把 5 丢掉 = 不筛，与 `?approval=9` 那条同族）。
pub(crate) fn anonymous_or_err(
    v: Option<i16>,
) -> DomainResult<Option<i16>> {
    match v {
        None => Ok(None),
        Some(n) if (0..=2).contains(&n) => Ok(Some(n)),
        Some(n) => Err(DomainError::Validation(format!(
            "匿名筛选取值无效：{n}（0 全部 / 1 仅匿名 / 2 仅具名）"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{anonymous_or_err, date_or_err, size_or_err};

    /// 空值仍是不筛（表单清空后照样提交空串，这是既有契约，不许变 400）
    #[test]
    fn empty_still_means_no_filter() {
        assert!(date_or_err(None).unwrap().is_none());
        assert!(date_or_err(Some("".into())).unwrap().is_none());
        assert!(date_or_err(Some("   ".into())).unwrap().is_none());
        assert!(size_or_err(Some("".into())).unwrap().is_none());
        assert!(anonymous_or_err(None).unwrap().is_none());
    }

    /// 合法值放行且语义不变
    #[test]
    fn valid_values_pass() {
        assert_eq!(date_or_err(Some("2026-10-01".into())).unwrap().as_deref(),
                   Some("2026-10-01"));
        assert_eq!(size_or_err(Some("1GB".into())).unwrap(),
                   Some(1024i64.pow(3)));
        assert_eq!(size_or_err(Some("1024".into())).unwrap(), Some(1024));
        for n in [0i16, 1, 2] {
            assert_eq!(anonymous_or_err(Some(n)).unwrap(), Some(n));
        }
    }

    /// 核心判据：给了值却解析不出来 ⇒ Err（旧口径是静默不筛 = 返回全站）
    #[test]
    fn garbage_is_rejected_not_ignored() {
        for bad in ["2025-13-45", "2025-02-30", "2025-1-5", "tomorrow"] {
            assert!(date_or_err(Some(bad.into())).is_err(),
                    "{bad} 必须 400，而不是当成没筛");
        }
        for bad in ["abc", "12XB", "-5MB", "0"] {
            assert!(size_or_err(Some(bad.into())).is_err(),
                    "{bad} 必须 400");
        }
        for n in [3i16, -1, 5] {
            assert!(anonymous_or_err(Some(n)).is_err(), "{n} 必须 400");
        }
        // 文案要能自证（会员看得到该填什么）
        let e = date_or_err(Some("2025-13-45".into())).unwrap_err();
        assert!(e.to_string().contains("YYYY-MM-DD"), "{}", e);
    }
}

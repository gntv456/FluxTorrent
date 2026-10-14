//! 促销档位归一（`?promo=` / 兼容层同名参数共用）。
//!
//! 从 `query.rs` 拆出并**收紧口径**（批七十四 P-277）：此前未知值走
//! `matches!(…) .then_some(s)` 与多选里的 `_ => {}`，结果是
//! 「给了值却不认识 ⇒ 归一成 None ⇒ 不加任何谓词 ⇒ 返回全站」。
//! 用户在界面上要的是「30% 促销的种」，拿到的是全站列表，还不报错——
//! 分类参数（`category_id`）的同族问题早先已改成 400，注释原话就是
//! 「静默丢掉等于返回全站，比 400 危险得多」，促销参数是漏网的那一支。
//!
//! 白名单必须与**显示侧**同源：`torrents/promo.rs` 的倍率表、角标表
//! (`EFFECTIVE_LABEL`) 与 `label()` 都认 `p30`，行上会挂「30%」角标，
//! 而旧筛选侧白名单里没有它 ⇒ 「角标看得见、按它筛不出来」（P-278），
//! 正是 `promo.rs` 开头声明要防的那类漂移。

use crate::errors::{DomainError, DomainResult};

/// 可选档位：`free`/`x2`/`half`/`p30` 是「族」档（可多选），
/// `any`=任意优惠、`none`=无优惠（与具体档互斥，混给时以具体档为准）。
/// `x2free`/`x2half` 是**合成档**＝两族交集（批八十二 P-291）：显示侧
/// `torrents/promo.rs` 的 `EFFECTIVE_LABEL`/`label()` 认 `x2free`/`x2half`
/// 两个角标（「2xFree」「2x50%」），筛选侧原先只有四个族档 ⇒ 角标看得见
/// 却按「既是 2X 又免费」筛不出来，正是 `torrents/promo.rs` 开头声明
/// 要防的那类漂移；
/// 用户控制面板第 4/6 档也只有在这一天才可能折对（折成 `free,x2` 并集
/// 会把「只免费」「只 2X」的种一起端出来＝给错数据）。
const KINDS: [&str; 8] = ["free", "x2", "half", "p30", "x2free", "x2half",
                          "any", "none"];
/// 可并列多选的具体档（含两个合成档）
const SPECIFICS: [&str; 6] = ["free", "x2", "half", "p30", "x2free",
                              "x2half"];

/// 归一 `?promo=`：空=不筛（`Ok(None)`）；任一不认识的值=报错（`Err`）。
///
/// 保留的既有归一行为：大小写与空格归一、保序去重、多选逗号串、
/// `any`/`none` 与具体档混给时按具体档。改变的只有「非法值」的去向：
/// 静默丢弃 → 400。
pub(crate) fn norm_promo(
    s: Option<String>,
) -> Result<Option<String>, String> {
    let raw = match s {
        Some(v) => v.trim().to_ascii_lowercase(),
        None => return Ok(None),
    };
    let parts: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    if parts.is_empty() {
        return Ok(None);
    }
    for p in &parts {
        if !KINDS.contains(p) {
            return Err(format!(
                "优惠档位无效：{p}（可用 {}）",
                KINDS.join("/")
            ));
        }
    }
    let mut specifics: Vec<&str> = Vec::new();
    for p in &parts {
        if SPECIFICS.contains(p) && !specifics.contains(p) {
            specifics.push(p);
        }
    }
    if specifics.is_empty() {
        // 只给了 any / none（或两者都给）：取第一个，语义就是那一条
        return Ok(Some(parts[0].to_string()));
    }
    if specifics.len() == 1 {
        return Ok(Some(specifics[0].to_string()));
    }
    Ok(Some(specifics.join(",")))
}

/// 端点用包装：把「非法档位」直接打成站内 400 信封（两处入口共用，
/// 保证界面与第三方工具不会各判一套；P-277 之前这里是静默返回全站）。
pub(crate) fn promo_or_err(
    s: Option<String>,
) -> DomainResult<Option<String>> {
    norm_promo(s).map_err(DomainError::Validation)
}

#[cfg(test)]
mod tests {
    use super::norm_promo;

    /// 合法单值原样通过（`p30` 是新补的那一档——显示侧早就认它）
    #[test]
    fn valid_single_values_pass_through() {
        for v in ["free", "x2", "half", "p30", "any", "none"] {
            assert_eq!(
                norm_promo(Some(v.into())).unwrap().as_deref(),
                Some(v),
                "单值 {v} 应原样通过"
            );
        }
        assert_eq!(norm_promo(None).unwrap(), None, "不给=不筛");
        assert_eq!(norm_promo(Some("".into())).unwrap(), None, "空串=不筛");
    }

    /// 未知值必须报错。旧断言 `promo_unknown_dropped` 断言的是
    /// 「未知 → None」，而 None 在 SQL 侧 = 不加谓词 = 返回全站，
    /// 那条测试是把洞固化成回归护栏（2026-10-14 批七十四改判据）。
    #[test]
    fn unknown_value_is_error_not_silence() {
        for v in ["weird", "p30x", "all", "30"] {
            assert!(norm_promo(Some(v.into())).is_err(), "{v} 必须报错");
        }
        let e = norm_promo(Some("weird".into())).unwrap_err();
        assert!(e.contains("weird") && e.contains("p30"), "文案要带可用档位");
    }

    /// 多选：大小写/空格归一、保序去重；any/none 与具体档混给时按具体档
    #[test]
    fn multi_normalized() {
        assert_eq!(
            norm_promo(Some("free,x2".into())).unwrap().as_deref(),
            Some("free,x2")
        );
        assert_eq!(
            norm_promo(Some(" HALF , p30,free ,half".into()))
                .unwrap()
                .as_deref(),
            Some("half,p30,free"),
            "保序去重"
        );
        assert_eq!(
            norm_promo(Some("any,none".into())).unwrap().as_deref(),
            Some("any"),
            "只给 any/none 时取第一个"
        );
    }

    /// 合成档（P-291）：面板第 4/6 档与「2xFree」角标都靠这两个 token，
    /// 缺任一侧就是「角标看得见、按它筛不出来」的漂移。
    #[test]
    fn composite_tiers_are_selectable() {
        for v in ["x2free", "x2half"] {
            assert_eq!(
                norm_promo(Some(v.into())).unwrap().as_deref(),
                Some(v),
                "合成档 {v} 必须原样通过"
            );
        }
        assert_eq!(
            norm_promo(Some("half,x2free".into())).unwrap().as_deref(),
            Some("half,x2free"),
            "合成档可参与多选"
        );
        for v in ["x2freee", "2xfree", "x2half2"] {
            assert!(norm_promo(Some(v.into())).is_err(), "{v} 仍须报错");
        }
    }

    /// 多选里混一个非法值也要整体报错（旧行为是丢掉那一项、少筛一档还不说）
    #[test]
    fn multi_with_one_bad_part_is_error() {
        assert!(norm_promo(Some("x2, free, junk".into())).is_err());
        assert!(norm_promo(Some("free,p300".into())).is_err());
    }
}

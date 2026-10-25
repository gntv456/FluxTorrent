//! 种子列表查询参数（M02）：ListQuery 与宽松反序列化助手。
//! 从 torrent_http/list.rs 按域拆出，供 list.rs / detail.rs 共用。

use serde::Deserialize;

mod param_strict;
mod promo_norm;
mod query_de;

// 档位归一搬到了 promo_norm.rs；入口与兼容层都按 `query::norm_promo` 引用，
// 所以这里必须 pub(crate) 重导出。`de_*` 那几个只给本模块的 derive 用，不重导出。
pub(crate) use param_strict::{anonymous_or_err, date_or_err, size_or_err};
pub(crate) use promo_norm::{norm_promo, promo_or_err};
use query_de::{
    de_bool_lenient, de_category_ids_lenient, de_opt_num_lenient,
    de_tag_ids_lenient,
};

#[derive(Deserialize)]
pub(super) struct ListQuery {
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) medium_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) grade_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) edition_id: Option<i32>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    pub(super) official: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    pub(super) include_dead: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    pub(super) include_unapproved: Option<bool>,
    pub(super) search: Option<String>,
    /// 搜索范围：0=标题(默认) 1=副标题/简介 3=发布者 4=IMDb
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) search_area: Option<i32>,
    /// 匹配模式：0=AND 模糊(默认) 2=精确
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) search_mode: Option<i32>,
    pub(super) sort: Option<String>,
    /// 标签筛选（T-04）：tag_dict.id，命中 tags 关联（旧单值键；多选见 tag_ids）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) tag_id: Option<i32>,
    /// 标签多选（0159 P1）：`tag_ids=1,3` 或重复参数；空/全非法 = 不筛
    #[serde(default, deserialize_with = "de_tag_ids_lenient")]
    pub(super) tag_ids: Vec<String>,
    /// 标签多选匹配模式：any=任一命中(默认) all=全部命中
    #[serde(default)]
    pub(super) tag_mode: Option<String>,
    /// 存活筛选（0102，NP inclbooked/vivisect 口径）：0=全部 1=仅活种 2=仅断种（覆盖 include_dead）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) alive: Option<i16>,
    /// 种子状态（0102，NP 口径）：seeding=当前做种 leeching=当前下载 completed=完成
    /// incomplete=未完成 notseeding=未做种（多值逗号串待扩展，先单值）
    #[serde(default)]
    pub(super) status: Option<String>,
    /// 审核状态（0102）：0=全部 1=通过 2=被拒（含未审需 see_banned，另走 include_unapproved）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) approval: Option<i16>,
    /// 分类多选（0088）：`category_ids`（原生键，重复参数成 seq）与 `category_id`
    /// （前端/NP 旧键名，单值或逗号串）统一收集——serde rename_all/alias 对
    /// query-string 的重复键行为不可靠，这里显式两个键各收一份再合并去重。
    #[serde(default, deserialize_with = "de_category_ids_lenient")]
    pub(super) category_ids: Vec<String>,
    /// category_id 旧键名的旁路收集（与上合并；空则不影响）
    #[serde(default, skip_serializing, rename = "category_id")]
    pub(super) category_id_alias: Option<String>,
    /// 翻页方向（0170 双向 keyset）：prev = 上一页（取游标之前的 limit 行）
    #[serde(default)]
    pub(super) dir: Option<String>,
    pub(super) cursor: Option<String>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) limit: Option<i64>,
    // ===== 高级搜索增强 =====
    /// 体积区间：支持 `1024`（字节）`500MB` `1.5GB` `2TB` 等带单位写法（见 parse_size）
    #[serde(default)]
    pub(super) size_min: Option<String>,
    #[serde(default)]
    pub(super) size_max: Option<String>,
    /// 发布时间区间（`YYYY-MM-DD`，按日历日口径，含边界当天）
    #[serde(default)]
    pub(super) date_from: Option<String>,
    #[serde(default)]
    pub(super) date_to: Option<String>,
    /// 做种数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) min_seeders: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) max_seeders: Option<i32>,
    /// 排除关键字（标题/简介均不得命中）
    #[serde(default)]
    pub(super) exclude: Option<String>,
    /// 优惠筛选：free / x2 / half / any / none（白名单校验）
    #[serde(default)]
    pub(super) promo: Option<String>,
    /// 发布者用户名（模糊匹配）
    #[serde(default)]
    pub(super) owner: Option<String>,
    /// 仅显示我发布的种子
    #[serde(default, deserialize_with = "de_bool_lenient")]
    pub(super) mine: Option<bool>,
    // ===== 高级搜索补齐（0118）：下载数/完成数区间 + 匿名发布 =====
    /// 下载数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) min_leechers: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) max_leechers: Option<i32>,
    /// 完成数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) min_completed: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) max_completed: Option<i32>,
    /// 匿名发布：0=全部(默认) 1=仅匿名 2=仅具名
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) anonymous: Option<i16>,
    /// 仅看我书签收藏的种（阶段三筛选粒度；1/true 生效）
    #[serde(default, deserialize_with = "de_bool_lenient")]
    pub(super) bookmarked: Option<bool>,
    /// 媒体评分下界（0283 P1-5）：0-10 一位小数；越界值丢弃（不筛）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) rating_min: Option<f64>,
}

/// 日期参数归一：严格 `YYYY-MM-DD` **且日历上真存在**，否则 `None`。
/// 只查逐位形状不够：`2025-02-30` / `2025-13-45` 形状全对却会在
/// `$n::date` 处炸（2026-10-10 实测）。
///
/// 注意口径分层：本函数返回 `None` 只是「解析不出」；**列表出口**对
/// 「用户给了值却解析不出」的最终裁定在 `query/param_strict.rs::date_or_err`
/// （批七十五 P-279）——空值=不筛、给了烂值=400 站内信封。早期注释写的
/// 「非法值直接丢弃、宁可不筛」已被 P-279 取代：静默不筛会让会员拿到
/// 全站还以为筛过了（同族：category_id/approval/promo 三处先例）。
pub(crate) fn norm_date(s: Option<String>) -> Option<String> {
    let s = s?;
    let s = s.trim();
    let shape_ok = s.len() == 10
        && s.as_bytes().iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    let real = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok();
    (shape_ok && real).then(|| s.to_string())
}

/// 标签参数归一（0159 P1）：tag_id（旧单值）与 tag_ids（多选/重复参数）合并，
/// 保序去重；tag_mode 只认 any/all，缺省 any。
pub(crate) fn norm_tags(
    tag_id: Option<i32>,
    tag_ids: &[String],
    tag_mode: Option<&str>,
) -> (Option<Vec<i32>>, bool) {
    let mut ids: Vec<i32> = Vec::new();
    if let Some(one) = tag_id {
        ids.push(one);
    }
    for part in tag_ids {
        for piece in part.split(',') {
            if let Ok(v) = piece.trim().parse::<i32>() {
                if !ids.contains(&v) {
                    ids.push(v);
                }
            }
        }
    }
    let all = matches!(tag_mode, Some("all"));
    ((!ids.is_empty()).then_some(ids), all)
}

/// 空白即视为未填（表单里清空后仍会提交空串）
pub(crate) fn norm_text(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 体积参数解析：`1024`（字节）/ `500MB` / `1.5GB` / `2TB`，KB/MB/GB/TB 与 KiB/GiB 写法均可，
/// 大小写不敏感。无效值或非正数返回 None（= 不筛，而不是报错）。
pub(crate) fn parse_size(s: Option<String>) -> Option<i64> {
    let raw = s?.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }
    let (num, mul) = if let Some(v) =
        raw.strip_suffix("tb").or_else(|| raw.strip_suffix("tib"))
    {
        (v, 1024f64.powi(4))
    } else if let Some(v) =
        raw.strip_suffix("gb").or_else(|| raw.strip_suffix("gib"))
    {
        (v, 1024f64.powi(3))
    } else if let Some(v) =
        raw.strip_suffix("mb").or_else(|| raw.strip_suffix("mib"))
    {
        (v, 1024f64.powi(2))
    } else if let Some(v) =
        raw.strip_suffix("kb").or_else(|| raw.strip_suffix("kib"))
    {
        (v, 1024f64)
    } else if let Some(v) = raw.strip_suffix('b') {
        (v, 1f64)
    } else {
        (raw.as_str(), 1f64)
    };
    let n: f64 = num.trim().parse().ok()?;
    let bytes = (n * mul).round();
    (bytes >= 1.0 && n.is_finite() && bytes < i64::MAX as f64)
        .then_some(bytes as i64)
}

#[cfg(test)]
mod tests {
    // 档位归一的判据随函数搬到 query/promo_norm.rs（P-277 收紧）
    use super::norm_date;

    /// 形状对但日历上不存在的日期必须丢弃：它们会在 PG 的 `$n::date`
    /// 处抛错，把列表高级搜索打成 500（2026-10-10 实测 date_from/date_to）。
    #[test]
    fn date_needs_a_real_calendar_day() {
        for bad in ["2025-13-45", "2025-02-30", "2025-06-31", "2025-00-10"] {
            assert!(
                norm_date(Some(bad.into())).is_none(),
                "{bad} 形状合法但日历上没有这一天"
            );
        }
        assert!(norm_date(Some("2024-02-29".into())).is_some(), "闰日要放行");
        assert!(norm_date(Some("2025-10-05".into())).is_some(), "正常日放行");
        assert!(norm_date(Some("2025-1-5".into())).is_none(), "必须零补齐");
    }
}

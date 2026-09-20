//! 种子列表查询参数（M02）：ListQuery 与宽松反序列化助手。
//! 从 torrent_http/list.rs 按域拆出，供 list.rs / detail.rs 共用。

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
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    pub(super) tag_id: Option<i32>,
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
}

/// 日期参数校验（严格 `YYYY-MM-DD`）：非法值直接丢弃——宁可不筛，
/// 也不让 PG 对烂字符串抛错变成 500。
pub(super) fn norm_date(s: Option<String>) -> Option<String> {
    let s = s?;
    let s = s.trim();
    let ok = s.len() == 10
        && s.as_bytes().iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    ok.then(|| s.to_string())
}

/// 优惠参数白名单（未知取值丢弃，避免静默返回空列表让用户以为「没数据」）
pub(super) fn norm_promo(s: Option<String>) -> Option<String> {
    let s = s?.trim().to_ascii_lowercase();
    matches!(s.as_str(), "free" | "x2" | "half" | "any" | "none").then_some(s)
}

/// 空白即视为未填（表单里清空后仍会提交空串）
pub(super) fn norm_text(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 体积参数解析：`1024`（字节）/ `500MB` / `1.5GB` / `2TB`，KB/MB/GB/TB 与 KiB/GiB 写法均可，
/// 大小写不敏感。无效值或非正数返回 None（= 不筛，而不是报错）。
pub(super) fn parse_size(s: Option<String>) -> Option<i64> {
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

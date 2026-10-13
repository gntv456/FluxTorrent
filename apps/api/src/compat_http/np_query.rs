//! NP 兼容列表端点的查询参数（300 行门禁从 nexusphp.rs 拆出，同
//! `rss_http/query.rs` 的做法）。
//!
//! **两套参数名并存**：本站名（`keyword`/`category`/`include_dead`/
//! `pagesize`）与 NexusPHP `torrents.php` 的惯用名（`search`/`cat`/
//! `incldead`/`rowset`）。批四十二实测（2026-10-13）：只认本站名时
//! `?search=片名` 与 `?cat=1` 被 serde 静默丢掉，端点回 **200 + 全站未筛
//! 列表**，`?incldead=0`（NP 口径「只要活种」）退化成含断种的全量 —— 刷流
//! 脚本据此批量收错种。这与批十一在 RSS 上修掉的是一模一样的病形
//! （见 `rss_http/query.rs` 头注），当时立下的口径就是「参数名靠 serde
//! alias 收口，别名层只做路径翻译」，兼容层两处不一致比两处都缺更糟。
//!
//! `sort=size&by=asc` 这类**排序**参数不在本轮 alias 范围：排序方向错了
//! 只影响顺序不影响集合，不构成「内容不对」。

/// `GET /compat/nexusphp/torrents.json` 的筛选参数。
#[derive(serde::Deserialize)]
pub struct NpListQuery {
    #[serde(default)]
    pub page: Option<i64>,
    /// 条数（NP 口径同时认 `rowset=`；RSS 侧的 `slot_number` 同名沿用）
    #[serde(default, alias = "rowset", alias = "slot_number")]
    pub pagesize: Option<i64>,
    /// 关键词（NP `torrents.php` 用 `search=`）
    #[serde(default, alias = "search")]
    pub keyword: Option<String>,
    /// 分类内部 id（NP 用 `cat=`；本站 RSS 早已同认此名）
    #[serde(default, alias = "cat")]
    pub category: Option<i32>,
    /// 搜索范围（NP 口径 0=标题 1=简介 3=发布者 4=IMDb）：主流工具
    /// （PT-Plugin-Plus 等）透传；NP 原名为无下划线的 `searcharea`
    #[serde(default, alias = "searcharea")]
    pub search_area: Option<i32>,
    /// 匹配模式（0=and 2=精确；NP 原名 `searchmode`）
    #[serde(default, alias = "searchmode")]
    pub search_mode: Option<i32>,
    /// IMDb 关键字（设置后等效 search_area=4）
    #[serde(default)]
    pub imdb: Option<String>,
    /// 存活筛选（0102 内部口径）：0=全部 1=仅活种 2=仅断种；
    /// **对外缺省 = 0（全部）** —— 新种在有人做种前也必须对第三方可见，
    /// 否则冷启动期工具搜索恒空（被误判成「对接坏了」）。
    #[serde(default)]
    pub alive: Option<i16>,
    /// 旧习惯兼容：1/true=全部（含断种）0/false=仅活种。给了 alive 时以
    /// alive 为准。取 String 而非 bool —— 老工具常发 `include_dead=1`，
    /// serde 的 bool 遇 "1" 会整条 Query 反序列化失败（此项目踩过同名坑）。
    /// NP 原名 `incldead`（无 e），不认它就会全站未筛（见头注）。
    #[serde(default, alias = "incldead")]
    pub include_dead: Option<String>,
    // ===== 0268：对外层透出站内已有的高级筛选（此前只有 9 个参数，
    // 「只看 free、>5GB、做种<3」这类刷流规则做不了；口径与网页搜索一致） =====
    /// 优惠筛选：free / x2 / half / any / none，支持逗号多选（与站内搜索同口径）
    #[serde(default)]
    pub promo: Option<String>,
    /// 体积下/上界：字节数或 `5GB`/`500MB`/`2TB`（与站内搜索同一解析器）
    #[serde(default)]
    pub size_min: Option<String>,
    #[serde(default)]
    pub size_max: Option<String>,
    /// 做种数下/上界（含）
    #[serde(default)]
    pub min_seeders: Option<i32>,
    #[serde(default)]
    pub max_seeders: Option<i32>,
    /// 发布时间范围（`YYYY-MM-DD`，含当天；非法值丢弃不筛）
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    /// 发布者用户名（模糊）
    #[serde(default)]
    pub owner: Option<String>,
    /// 官种：1/true = 仅官种
    #[serde(default)]
    pub official: Option<String>,
    /// 排序：created(默认)/seeders/size/completed，可加 `_asc` 反转
    #[serde(default)]
    pub sort: Option<String>,
    /// 标签：tag_dict.id 逗号串；`tag_mode=any|all`（缺省 any）
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub tag_mode: Option<String>,
}

/// 解析对外存活口径：alive 优先，其次 include_dead，最后缺省「全部」。
pub fn resolve_alive(alive: Option<i16>, include_dead: Option<&str>) -> i16 {
    if let Some(v) = alive {
        if (0..=2).contains(&v) {
            return v;
        }
    }
    match include_dead.map(str::trim) {
        Some("0") | Some("false") | Some("no") => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_alive, NpListQuery};

    fn parse(q: &str) -> NpListQuery {
        serde_json::from_str(q).unwrap()
    }

    /// NP 惯用名与本站名双收（批四十二判据；alias 在 derive 层生效，
    /// 与 query 编码格式无关 —— 同 rss_http::query::RssQuery 的既有做法）
    #[test]
    fn np_and_native_param_names() {
        let q =
            parse(r#"{"search":"大电影","cat":5,"incldead":"0","rowset":25}"#);
        assert_eq!(q.keyword.as_deref(), Some("大电影"));
        assert_eq!(q.category, Some(5));
        assert_eq!(q.include_dead.as_deref(), Some("0"));
        assert_eq!(q.pagesize, Some(25));
        let q2 = parse(
            r#"{"keyword":"大电影","category":5,"include_dead":"0",
                "pagesize":25,"searcharea":4,"searchmode":2}"#,
        );
        assert_eq!(q2.keyword.as_deref(), Some("大电影"));
        assert_eq!((q2.category, q2.pagesize), (Some(5), Some(25)));
        assert_eq!((q2.search_area, q2.search_mode), (Some(4), Some(2)));
    }

    /// `incldead=0` 是 NP 的「只要活种」，必须落到 alive=1 而不是缺省全部
    #[test]
    fn incldead_zero_means_alive_only() {
        assert_eq!(
            resolve_alive(
                parse(r#"{"incldead":"0"}"#).alive,
                parse(r#"{"incldead":"0"}"#).include_dead.as_deref()
            ),
            1
        );
        let q = parse(r#"{"incldead":"1"}"#);
        assert_eq!(resolve_alive(q.alive, q.include_dead.as_deref()), 0);
        assert_eq!(resolve_alive(None, None), 0);
        assert_eq!(resolve_alive(Some(2), Some("0")), 2);
    }
}

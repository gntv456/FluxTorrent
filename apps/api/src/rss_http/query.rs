//! RSS 查询参数（300 行门禁从 rss_http.rs 拆出）。
//!
//! 两套命名并存：本站名（`categories`/`showrows`）与 NexusPHP
//! `getrss.php` 惯用名（`cat`/`slot_number`）。别名层（compat_http::
//! aliases）只做「路径翻译」，参数名靠这里的 serde alias 收口——
//! 2026-10-10 批十一实测：`/rss/{pk}?cat=5` 与 `getrss.php?cat=5` 都把
//! 筛选静默丢掉退化成全站 50 条并回 200，只认 NP 名的老脚本拿到的是
//! 「筛选没报错但内容不对」的订阅源，比拒单更难发现。

/// `GET /rss/{passkey}` 的筛选参数。
#[derive(serde::Deserialize)]
pub struct RssQuery {
    /// 旧版单值兼容（NP 口径同时认 `cat=`）
    #[serde(alias = "cat")]
    pub category: Option<i32>,
    pub categories: Option<String>,
    pub mediums: Option<String>,
    pub official: Option<bool>,
    pub search: Option<String>,
    /// 条数（NP 口径同时认 `slot_number=`）
    #[serde(alias = "slot_number")]
    pub showrows: Option<i64>,
    /// dl = 标题带元信息（默认）；page = 仅标题
    pub linktype: Option<String>,
    /// 0=全部（含未购付费种） 1=仅免费促销（free/x2free；与同类站 paid 口径对齐）；
    /// 缺省 = 排除「付费且未购」的种子（深测 2026-10-03：付费种无差别推送会让
    /// 刷流器反复拿到 402，默认挡掉；需要全量监控付费种的用户显式传 paid=0）
    pub paid: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::RssQuery;

    /// NP 惯用名与本站名双收（alias 在 derive 层生效，与编码格式无关）
    #[test]
    fn np_and_native_param_names() {
        let q: RssQuery =
            serde_json::from_str(r#"{"cat":5,"slot_number":3}"#).unwrap();
        assert_eq!(q.category, Some(5));
        assert_eq!(q.showrows, Some(3));
        let q2: RssQuery =
            serde_json::from_str(r#"{"category":6,"showrows":4}"#).unwrap();
        assert_eq!((q2.category, q2.showrows), (Some(6), Some(4)));
    }
}

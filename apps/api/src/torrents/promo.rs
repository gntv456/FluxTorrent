//! 优惠（promotions）匹配口径的**唯一来源**：命中条件 + 优先级排序 + 常用片段。
//!
//! 背景（2026-09-22 六维强化方案 P0-5）：此前促销判定散在三处——
//! `list.rs` 的 PROMO_MATCH 常量、`list_noclamp_as.rs` 列表 SELECT 内联
//! （还重复写了两遍）、`rss_http.rs` 独立内联——改一种促销作用域要同步改三处，
//! 容易出现「列表徽标显示免费、但按免费筛选筛不出来」这类漂移。
//! 现在全部引用本模块。
//!
//! 口径：种子级直挂（`torrent_id = t.id`）或作用域级
//! （global / official / non_official / category），且处于生效时间窗内；
//! 同一粒子上命中多条时按 kind 优先级取最高的一条。

/// 促销 kind 优先级（数值越大越优先）：同一粒子命中多条促销时展示哪一条。
/// 顺序：双免 > 2x半价 > 2x上传 > 免费 > 半价 > 30%下载 > 其它。
pub(crate) const KIND_RANK: &str = "CASE p.kind::text \
     WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
     WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END";

/// 促销命中条件（促销表别名固定为 `p`、种子表别名固定为 `t`）。
pub(crate) fn matched() -> String {
    "p.starts_at <= now() AND p.ends_at > now() AND (\
     p.torrent_id = t.id OR (p.torrent_id IS NULL AND (\
       p.scope = 'global' \
       OR (p.scope = 'official' AND t.official_tag) \
       OR (p.scope = 'non_official' AND NOT t.official_tag) \
       OR (p.scope = 'category' AND t.category_id = p.category_id))))"
        .to_string()
}

/// 列表/详情取「命中的最高优先级促销」：一次 LATERAL 同时拿到 kind 与结束时间。
/// 替代此前两条除 SELECT 列外完全相同的 correlated 子查询。
pub(crate) fn lateral_latest() -> String {
    format!(
        "LEFT JOIN LATERAL (SELECT p.kind::text AS promotion, \
         p.ends_at AS promotion_ends_at \
         FROM promotions p WHERE {} \
         ORDER BY {} DESC, p.id DESC LIMIT 1) pr ON TRUE",
        matched(),
        KIND_RANK
    )
}

/// 谓词用：是否存在命中促销；`kinds` 给定时额外限定促销类型集合
/// （入参是已带引号的 SQL 字面量，如 `'free','x2free'`）。
pub(crate) fn exists_clause(kinds: Option<&str>) -> String {
    match kinds {
        Some(k) => format!(
            "EXISTS(SELECT 1 FROM promotions p \
             WHERE {} AND p.kind::text IN ({k}))",
            matched()
        ),
        None => {
            format!("EXISTS(SELECT 1 FROM promotions p WHERE {})", matched())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 四种作用域 + 种子级直挂 + 时间窗，一个都不能少
    /// （少了就会出现「徽标显示免费但筛不出来」）。
    #[test]
    fn matched_covers_all_scopes() {
        let m = matched();
        assert!(m.contains("p.torrent_id = t.id"), "缺种子级直挂");
        assert!(m.contains("p.scope = 'global'"), "缺全局作用域");
        assert!(m.contains("p.scope = 'official'"), "缺官种作用域");
        assert!(m.contains("p.scope = 'non_official'"), "缺非官种作用域");
        assert!(m.contains("p.scope = 'category'"), "缺分类作用域");
        assert!(m.contains("p.starts_at <= now()"), "缺生效下界");
        assert!(m.contains("p.ends_at > now()"), "缺生效上界");
    }

    /// kind 优先级六档必须齐全且顺序不变
    /// —— 这是「同一粒子多促销取哪条」的唯一裁决点。
    #[test]
    fn kind_rank_order_is_stable() {
        let r = KIND_RANK;
        let pos = |k: &str| r.find(k).unwrap_or_else(|| panic!("缺 {k}"));
        let (x2free, x2half, x2, free, half, p30) = (
            pos("'x2free'"),
            pos("'x2half'"),
            pos("'x2'"),
            pos("'free'"),
            pos("'half'"),
            pos("'p30'"),
        );
        let ordered = x2free < x2half
            && x2half < x2
            && x2 < free
            && free < half
            && half < p30;
        assert!(ordered, "kind 优先级顺序被改动：{r}");
        assert!(
            r.contains("THEN 6") && r.contains("THEN 1"),
            "优先级数值区间变了"
        );
    }

    /// LATERAL 片段必须只出现一次时间窗判定、且带 LIMIT 1
    /// （避免多行 JOIN 导致列表行数翻倍）。
    #[test]
    fn lateral_takes_single_row() {
        let l = lateral_latest();
        assert!(l.contains("LEFT JOIN LATERAL"), "不是 LATERAL 连接");
        assert!(l.contains("LIMIT 1"), "缺 LIMIT 1，会出现重复行");
        assert_eq!(l.matches("p.ends_at > now()").count(), 1, "时间窗重复");
        assert!(l.contains("AS promotion_ends_at"), "未同时取结束时间");
    }

    /// kind 限定集合要真的进 SQL
    /// （筛选 free 必须包含 x2free，否则双免种筛不出来）。
    #[test]
    fn exists_clause_filters_kinds() {
        let free = exists_clause(Some("'free','x2free'"));
        assert!(free.starts_with("EXISTS("));
        assert!(free.contains("p.kind::text IN ('free','x2free')"));
        let any = exists_clause(None);
        assert!(!any.contains("p.kind::text IN"), "any 不应限定 kind");
        assert!(any.contains("p.torrent_id = t.id"));
    }
}

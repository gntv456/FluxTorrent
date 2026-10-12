//! 优惠（promotions）匹配口径的**唯一来源**：命中条件 + 多维合并 + 常用片段。
//!
//! 背景（2026-09-22 六维强化方案 P0-5）：此前促销判定散在三处——
//! `list.rs` 的 PROMO_MATCH 常量、`list_noclamp_as.rs` 列表 SELECT 内联
//! （还重复写了两遍）、`rss_http.rs` 独立内联——改一种促销作用域要同步改三处，
//! 容易出现「列表徽标显示免费、但按免费筛选筛不出来」这类漂移。
//! 现在全部引用本模块。
//!
//! 口径：种子级直挂（`torrent_id = t.id`）或作用域级
//! （global / official / non_official / category），且处于生效时间窗内。
//!
//! **同一粒子命中多条时按维度合并**（批三十七 P-120 配套，与 worker
//! `billing::billing_multipliers` 同口径）：上传倍率取最大、下载倍率取最小，
//! 再把合成后的倍率反查回档位标签。旧写法是「按强度序取最强的一条」，
//! 把两个正交维度压成一个标量，于是全站 x2 活动期间单种 free 的角标会写成
//! `x2`、计费也照 1× 下（免流被吞）。表外组合（x2 + p30 ⇒ 2×/0.3×）
//! 取折扣侧标签（`p30`）——宁可少报一个上传翻倍，也不误报下载倍率。

/// 上传倍率片段（命中多条时取 `max`）
const UP_FACTOR: &str = "CASE p.kind::text WHEN 'x2' THEN 2 \
     WHEN 'x2free' THEN 2 WHEN 'x2half' THEN 2 ELSE 1 END";

/// 下载倍率片段（命中多条时取 `min`；千分位整数避免浮点比较）
const DOWN_FACTOR: &str = "CASE p.kind::text WHEN 'free' THEN 0 \
     WHEN 'x2free' THEN 0 WHEN 'half' THEN 5 WHEN 'x2half' THEN 5 \
     WHEN 'p30' THEN 3 ELSE 10 END";

/// 合成倍率 → 档位标签（`x` 是聚合后的子查询别名，列为 `mu`/`du`/`ends_at`）。
/// 没有 ELSE：走到 ELSE 说明六档表外还有组合（宁缺不假），出 NULL = 不挂角标。
const EFFECTIVE_LABEL: &str = "CASE WHEN x.du = 0 AND x.mu = 2 THEN 'x2free' \
     WHEN x.du = 0 THEN 'free' WHEN x.du = 5 AND x.mu = 2 THEN 'x2half' \
     WHEN x.du = 5 THEN 'half' WHEN x.du = 3 THEN 'p30' \
     WHEN x.mu = 2 THEN 'x2' END";

/// 聚合内核：一次扫出「上传最大 / 下载最小 / 最早结束」。
/// `min(ends_at)` = 合成效果**下一次改变**的时刻（多条促销里最早到期那条）。
fn agg() -> String {
    format!(
        "(SELECT max({UP_FACTOR}) AS mu, min({DOWN_FACTOR}) AS du, \
         min(p.ends_at) AS ends_at FROM promotions p WHERE {})",
        matched()
    )
}

/// 只保留「真的有流量效果」的合成行（mu>1 或 du<10）；
/// 全部命中档都是未知 kind（倍率 1×1）时出 0 行 ⇒ 角标为空。
const EFFECTIVE_FILTER: &str = "WHERE x.mu > 1 OR x.du < 10";

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

/// 列表/RSS 取「命中的全部促销按维度合成后的有效档位」：
/// 一次 LATERAL 同时拿到 kind 与结束时间。
pub(crate) fn lateral_latest() -> String {
    format!(
        "LEFT JOIN LATERAL (SELECT {EFFECTIVE_LABEL} AS promotion, \
         x.ends_at AS promotion_ends_at FROM {} x {EFFECTIVE_FILTER}) \
         pr ON TRUE",
        agg()
    )
}

/// 标量子查询版有效档位（给不便 JOIN LATERAL 的语句：详情/保种面板/后台列表）
pub(crate) fn effective_kind() -> String {
    format!(
        "(SELECT {EFFECTIVE_LABEL} FROM {} x {EFFECTIVE_FILTER})",
        agg()
    )
}

/// 标量子查询版结束时间（与 [`effective_kind`] 同一份聚合，不会两处漂移）
pub(crate) fn effective_ends() -> String {
    format!("(SELECT x.ends_at FROM {} x {EFFECTIVE_FILTER})", agg())
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

/// 促销档位的对外展示标签：RSS 标题前缀、Torznab tags、compat 促销名共用
/// 同一份，防止三处各写一份而漂移（此前 rss_http.rs 与 economy_http
/// 的 recon.rs 各有一份）。未知/无促销 → 空串。
pub fn label(kind: &str) -> &'static str {
    match kind {
        "free" => "Free",
        "x2free" => "2xFree",
        "half" => "50%",
        "x2half" => "2x50%",
        "x2" => "2x",
        "p30" => "30%",
        _ => "",
    }
}

/// 促销档位的 (下载倍率, 上传倍率) —— Torznab 的
/// downloadvolumefactor / uploadvolumefactor 口径。
/// 语义与 worker 计费、0213 effect 注释一致：
/// free 下载 0 / x2 上传 2 / x2free 0+2 / half 下载 0.5 /
/// x2half 0.5+2 / p30 下载 0.3；无促销 → (1,1)。
pub fn factors(kind: &str) -> (f64, f64) {
    match kind {
        "free" => (0.0, 1.0),
        "x2free" => (0.0, 2.0),
        "half" => (0.5, 1.0),
        "x2half" => (0.5, 2.0),
        "x2" => (1.0, 2.0),
        "p30" => (0.3, 1.0),
        _ => (1.0, 1.0),
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

    /// 六档的倍率片段必须逐档齐全（P-120 起「取哪条」不再是优先级排序，
    /// 而是逐维合并——片段少一档就等于那一档被当成 1×1 计费）
    #[test]
    fn factor_tables_cover_six_tiers() {
        for k in ["'free'", "'x2'", "'x2free'", "'half'", "'x2half'", "'p30'"]
        {
            assert!(
                UP_FACTOR.contains(k) || DOWN_FACTOR.contains(k),
                "倍率片段缺档 {k}"
            );
        }
        assert_eq!(UP_FACTOR.matches("THEN 2").count(), 3, "翻倍档少了");
        assert_eq!(DOWN_FACTOR.matches("THEN 0").count(), 2, "免流档少了");
        assert_eq!(DOWN_FACTOR.matches("THEN 5").count(), 2, "半价档少了");
        assert!(DOWN_FACTOR.contains("THEN 3"), "p30 档没了");
        assert!(DOWN_FACTOR.contains("ELSE 10"), "未知档必须按 1× 计");
    }

    /// 合成标签：先判「下载 0」再看上传，x2+p30 这类表外组合折回折扣侧
    /// （宁可少报一个上传翻倍，也不虚报下载倍率）
    #[test]
    fn effective_label_maps_combinations() {
        let l = EFFECTIVE_LABEL;
        assert!(l.contains("x.du = 0 AND x.mu = 2 THEN 'x2free'"));
        assert!(l.contains("x.du = 5 AND x.mu = 2 THEN 'x2half'"));
        assert!(l.contains("x.du = 3 THEN 'p30'"), "p30 组合档要取折扣侧");
        assert!(!l.contains("ELSE"), "表外组合必须出 NULL，不能编一个档位");
    }

    /// LATERAL 一次聚合出两列，且**至多一行**：旧写法靠 `ORDER BY 强度
    /// LIMIT 1`，现在靠无 GROUP BY 的聚合恒一行 + 零效果行过滤器
    /// （过滤器漏了，列表每行都会挂出一个 `promotion = NULL` 的重复 JOIN 行）
    #[test]
    fn lateral_takes_single_row() {
        let l = lateral_latest();
        assert!(l.contains("LEFT JOIN LATERAL"), "不是 LATERAL 连接");
        assert!(l.contains("AS mu") && l.contains("AS du"), "没按维度聚合");
        assert!(
            l.contains("WHERE x.mu > 1 OR x.du < 10"),
            "缺零效果过滤，会出现 promotion 为空的多余行"
        );
        assert!(!l.contains("LIMIT"), "聚合已保证单行，不该再有 LIMIT");
        assert_eq!(l.matches("p.ends_at > now()").count(), 1, "时间窗重复");
        assert!(l.contains("AS promotion_ends_at"), "未同时取结束时间");
    }

    /// 三个出口（LATERAL / 标量档位 / 标量结束时间）共用同一份聚合片段
    #[test]
    fn all_three_outlets_share_one_aggregate() {
        let a = agg();
        assert!(lateral_latest().contains(&a), "LATERAL 用了另一份聚合");
        assert!(effective_kind().contains(&a), "标量档位用了另一份聚合");
        assert!(effective_ends().contains(&a), "标量结束用了另一份聚合");
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

    /// 六档标签齐备且不误伤未知档位（第三方按标签过滤，缺档 = 免费信息断传）
    #[test]
    fn label_covers_all_kinds() {
        assert_eq!(label("free"), "Free");
        assert_eq!(label("x2free"), "2xFree");
        assert_eq!(label("half"), "50%");
        assert_eq!(label("x2half"), "2x50%");
        assert_eq!(label("x2"), "2x");
        assert_eq!(label("p30"), "30%");
        assert_eq!(label(""), "");
        assert_eq!(label("sticky1"), "", "置顶档位没有流量标签");
    }

    /// 倍率必须与 0213 的 effect 语义逐档对齐（错一档 = 媒体库免费判断出错）
    #[test]
    fn factors_match_billing_semantics() {
        assert_eq!(factors("free"), (0.0, 1.0));
        assert_eq!(factors("x2"), (1.0, 2.0));
        assert_eq!(factors("x2free"), (0.0, 2.0));
        assert_eq!(factors("half"), (0.5, 1.0));
        assert_eq!(factors("x2half"), (0.5, 2.0));
        assert_eq!(factors("p30"), (0.3, 1.0));
        assert_eq!(factors("nope"), (1.0, 1.0));
    }
}

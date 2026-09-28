//! 同义词检索（E9）：site_terms 替换词 → 规范词的搜索扩展。
//! 模式字符白名单（字母数字/CJK/空白），其余字符整条丢弃——LIKE 通配与
//! 引号不可能进入 SQL；上限 5 条防规则库撑爆查询。无命中时零改动。

use sqlx::PgPool;

/// 搜索词（或其前缀）命中 site_terms.replacement 时，返回对应 canonical 的
/// LIKE 模式。
pub(super) async fn synonym_patterns(
    db: &PgPool,
    search: Option<&str>,
    exact: bool,
) -> Vec<String> {
    let Some(q) = search.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT canonical, replacement FROM site_terms \
         WHERE enabled LIMIT 200",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let safe = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| c.is_alphanumeric() || c.is_whitespace())
    };
    let mut out: Vec<String> = Vec::new();
    for (canonical, replacement) in rows {
        // 命中：搜索词包含整条替换词，或替换词以 ≥2 字符的搜索词为前缀
        let hit = q.contains(&replacement)
            || (replacement.len() > 2
                && replacement.starts_with(q)
                && q.len() >= 2);
        if !hit || !safe(&canonical) || canonical == q {
            continue;
        }
        let pat = if exact {
            canonical.clone()
        } else {
            format!("%{}%", canonical)
        };
        if !out.contains(&pat) {
            out.push(pat);
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

/// 把同义词模式拼成 OR 段（无模式返回空串）。模式经白名单，可安全内联。
pub(super) fn build_syn_pred(
    pats: &[String],
    search_area: Option<i32>,
    esc: &str,
) -> String {
    if pats.is_empty() {
        return String::new();
    }
    let cols: &[&str] = match search_area.unwrap_or(0) {
        1 | 4 => &["t.small_descr", "t.descr"],
        _ => &["t.name", "t.small_descr", "t.descr"],
    };
    let likes: Vec<String> = pats
        .iter()
        .flat_map(|p| {
            cols.iter()
                .map(move |c| format!("{c} ILIKE '{p}'{esc}"))
                .collect::<Vec<_>>()
        })
        .collect();
    format!(" OR ({})", likes.join(" OR "))
}

/// 主搜索谓词 + 同义词扩展段（E9 合成出口）：无同义词命中时输出与
/// 原谓词逐字节一致，零行为变化。
pub(super) async fn search_pred_with_syn(
    db: &PgPool,
    search: Option<&str>,
    search_area: Option<i32>,
    exact: bool,
    esc: &str,
) -> String {
    let pats = synonym_patterns(db, search, exact).await;
    let syn_pred = build_syn_pred(&pats, search_area, esc);
    match search_area.unwrap_or(0) {
        1 => format!(
            "AND ($6::text IS NULL OR t.small_descr ILIKE $6{esc} \
             OR t.descr ILIKE $6{esc}){syn_pred}"
        ),
        3 => format!("AND ($6::text IS NULL OR u.username ILIKE $6{esc})"),
        4 => format!(
            "AND ($6::text IS NULL OR t.media_info->>'imdb' ILIKE $6{esc} \
             OR t.descr ILIKE $6{esc}){syn_pred}"
        ),
        _ => format!(
            "AND ($6::text IS NULL OR t.name ILIKE $6{esc} \
             OR t.small_descr ILIKE $6{esc} OR t.descr ILIKE $6{esc} \
             OR t.id IN (SELECT torrent_id FROM files \
             WHERE path ILIKE $6{esc})){syn_pred}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::build_syn_pred;

    /// 拼装形状：默认范围三列 OR、整体带 OR 前缀
    #[test]
    fn syn_pred_shape() {
        let p = build_syn_pred(&["%种子%".into()], None, "");
        assert!(p.contains("t.name ILIKE '%种子%'"));
        assert!(p.contains("t.small_descr ILIKE '%种子%'"));
        assert!(p.starts_with(" OR ("));
    }

    /// 范围 1（副题/简介）不搜标题
    #[test]
    fn syn_pred_area1_skips_name() {
        let p = build_syn_pred(&["%x%".into()], Some(1), "");
        assert!(!p.contains("t.name"));
    }

    #[test]
    fn empty_patterns_empty_pred() {
        assert_eq!(build_syn_pred(&[], None, ""), "");
    }
}

//! Section 多维 SQL 助手（第八轮 0102 起）：从 list_noclamp_as.rs 拆出，守 300 行。
//! 三件事：
//!  1) `section_where`：`sec_{kind}=dict_id` 的列表谓词（同维度多值 OR，跨维度 AND）；
//!  2) `legacy_filter_dual`：`medium_id=/grade_id=/edition_id=` 旧列谓词的双路版；
//!  3) `sync_legacy_columns`：写 sections 后按**名称**反向回填旧三列。
//!
//! 为什么必须按名称映射：`torrents.medium_id`（media 表 id，1..8）与
//! `section_dict.id`（自增，0174 回填后已到 400+ 段）是**两个互不相通的 id 空间**。
//! 直接把同一个数组同时比列和比 dict_id（rss_http 旧写法就这样）在新装站上会
//! 命中毫不相干的字典行，属于假双路。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 旧三列 ↔ sections 维度的对应关系：(kind, 旧列, 旧实体表)
const LEGACY_DIMS: [(&str, &str, &str); 3] = [
    ("media", "t.medium_id", "media"),
    ("grades", "t.grade_id", "grades"),
    ("editions", "t.edition_id", "editions"),
];

fn legacy_of(kind: &str) -> Option<(&'static str, &'static str)> {
    LEGACY_DIMS
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, col, tbl)| (*col, *tbl))
}

/// 拼装 ` AND (...)` 片段；无有效维度时返回空串。
/// kind 以 section_kinds 存在性校验，dict_id 为 i64 内插（无注入面）。
pub(crate) async fn section_where(
    db: &PgPool,
    sections: &[(String, i64)],
) -> String {
    if sections.is_empty() {
        return String::new();
    }
    use std::collections::BTreeMap;
    let mut by_kind: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (kind, dict_id) in sections {
        by_kind.entry(kind.as_str()).or_default().push(*dict_id);
    }
    let mut out = String::new();
    for (kind, ids) in by_kind {
        if !crate::admin_p3_http::is_custom_kind(db, kind).await {
            continue;
        }
        let list = ids
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let arr = format!("ARRAY[{list}]::bigint[]");
        let sections_hit = format!(
            "t.id IN (SELECT torrent_id FROM torrent_sections \
             WHERE kind = '{kind}' AND dict_id = ANY({arr}))"
        );
        match legacy_of(kind) {
            // 旧三列维度：历史种子只有列值、新种子只有 sections，两边都要命中
            Some((col, tbl)) => out.push_str(&format!(
                " AND ({sections_hit} OR {col} IN (SELECT l.id FROM {tbl} l \
                 JOIN section_dict sd ON sd.kind = '{kind}' AND sd.name = l.name \
                 WHERE sd.id = ANY({arr})))"
            )),
            None => out.push_str(&format!(" AND {sections_hit}")),
        }
    }
    out
}

/// 旧列筛选（`medium_id=$2` 等）的双路版：给定的 `$param` 是**旧实体表 id**，
/// 既比列，也按名称去找挂在同名字典项上的 sections 种子。
/// 返回形如 `($2::int IS NULL OR t.medium_id = $2 OR EXISTS (...))`。
pub(crate) fn legacy_filter_dual(
    kind: &str,
    param: &str,
) -> Option<String> {
    let (col, tbl) = legacy_of(kind)?;
    Some(format!(
        "({param}::int IS NULL OR {col} = {param} OR EXISTS (SELECT 1 \
         FROM torrent_sections ts JOIN section_dict sd ON sd.id = ts.dict_id \
         WHERE ts.torrent_id = t.id AND ts.kind = '{kind}' \
         AND sd.name = (SELECT name FROM {tbl} WHERE id = {param})))"
    ))
}

/// 写 sections 之后按名称反向回填旧三列，让「按媒介/学段/版本筛选」与
/// 「列表按旧列翻译」不再漏掉只写了 sections 的种子。
///
/// 该维度在本站存在时以 sections 为准（没挂就等于清空旧列）；维度已被站长删除时
/// 不碰旧列——否则会把历史值抹成 NULL。
pub(crate) async fn sync_legacy_columns(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<()> {
    let mut sets: Vec<String> = Vec::new();
    for (kind, col, tbl) in LEGACY_DIMS {
        let bare = col.replace("t.", "");
        sets.push(format!(
            "{bare} = CASE WHEN EXISTS (SELECT 1 FROM section_kinds WHERE kind = '{kind}') \
             THEN (SELECT l.id FROM torrent_sections ts \
                   JOIN section_dict sd ON sd.id = ts.dict_id \
                   JOIN {tbl} l ON l.name = sd.name \
                   WHERE ts.torrent_id = $1 AND ts.kind = '{kind}') \
             ELSE {bare} END"
        ));
    }
    sqlx::query(&format!(
        "UPDATE torrents SET {} WHERE id = $1",
        sets.join(", ")
    ))
    .bind(torrent_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{legacy_filter_dual, legacy_of};

    #[test]
    fn legacy_dims_map_only_three_kinds() {
        assert!(legacy_of("media").is_some());
        assert!(legacy_of("grades").is_some());
        assert!(legacy_of("editions").is_some());
        // 自建维度（如 team/codec）没有旧列，走单路 sections
        assert!(legacy_of("team").is_none());
        assert!(legacy_of("codec").is_none());
    }

    #[test]
    fn dual_filter_names_both_sides() {
        let s = legacy_filter_dual("media", "$2").unwrap();
        assert!(s.contains("t.medium_id = $2"), "{s}");
        assert!(s.contains("ts.kind = 'media'"), "{s}");
        assert!(s.contains("FROM media WHERE id = $2"), "{s}");
        assert!(legacy_filter_dual("team", "$9").is_none());
    }
}

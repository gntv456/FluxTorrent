//! 标签筛选谓词（0159 P1 多选）：any = 单谓词 = ANY（复用 idx_tags_tag）；
//! all = 命中去重后等于标签数（聚合子查询，仍走索引）。
//!
//! 槽位由调用方传入：列表侧 $9、计数侧 $8 不同。此前谓词把 $9 硬编码并
//! 整段插进计数 SQL，与计数侧 size_min（bigint）撞同一槽 → prepare 失败
//! 被 unwrap_or(0) 吞成「列表有行、总数恒 0」。
pub(super) fn tag_pred(
    tag_ids: &Option<Vec<i32>>,
    tag_all: bool,
    slot: usize,
) -> String {
    match (tag_ids, tag_all) {
        (None, _) => String::new(),
        (Some(_), false) => format!(
            " AND t.id IN (SELECT torrent_id FROM tags \
             WHERE tag_id = ANY(${slot}::int[]))"
        ),
        (Some(ids), true) => format!(
            " AND (SELECT count(DISTINCT tag_id) FROM tags WHERE \
             torrent_id = t.id AND tag_id = ANY(${slot}::int[])) = {}",
            ids.len()
        ),
    }
}

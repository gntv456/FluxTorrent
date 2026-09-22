//! 种子列表 viewer 维度谓词（从 list_noclamp_as.rs 按域拆出，300 行门禁）：
//! 种子状态（0102 snatches 存在性）与书签筛选（阶段三筛选粒度）。
//! viewer 由调用方注入 SQL 文本（i64 字符串，无注入面）。

/// 种子状态谓词：seeding/leeching/completed/incomplete/notseeding
/// （0102，NP 口径；viewer=0 无人视角时全部恒空——上层保证 viewer 是真实 uid）。
pub(super) fn status_pred(status: Option<&str>, viewer_sql: &str) -> String {
    let base = match status {
        Some("seeding") => {
            " AND EXISTS(SELECT 1 FROM snatches s \
             WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)"
        }
        Some("leeching") => {
            " AND EXISTS(SELECT 1 FROM snatches s \
             WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.leeching)"
        }
        Some("completed") => {
            " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id \
             AND s.user_id = {viewer} AND s.completed_at IS NOT NULL)"
        }
        Some("incomplete") => {
            " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id \
             AND s.user_id = {viewer} AND s.completed_at IS NULL \
             AND (s.uploaded > 0 OR s.downloaded > 0))"
        }
        Some("notseeding") => {
            " AND NOT EXISTS(SELECT 1 FROM snatches s \
             WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)"
        }
        _ => "",
    };
    base.replace("{viewer}", viewer_sql)
}

/// 书签谓词（阶段三筛选粒度）：bookmarked=1 只看我收藏的种。
/// 倒序（未收藏）无竞品先例，不做。
pub(super) fn bookmark_pred(viewer_sql: &str) -> String {
    " AND EXISTS(SELECT 1 FROM bookmarks b \
     WHERE b.torrent_id = t.id AND b.user_id = {viewer})"
        .replace("{viewer}", viewer_sql)
}

#[cfg(test)]
mod tests {
    use super::{bookmark_pred, status_pred};

    /// 五种状态都要带 viewer 且语义正确（存在/不存在）
    #[test]
    fn status_pred_shapes() {
        let s = status_pred(Some("seeding"), "42");
        assert!(s.contains("s.user_id = 42") && s.contains("s.seeding"));
        assert!(!s.contains("NOT EXISTS"), "seeding 是肯定存在");
        let n = status_pred(Some("notseeding"), "42");
        assert!(n.contains("NOT EXISTS") && n.contains("s.seeding"));
        assert!(status_pred(None, "42").is_empty(), "无状态不过滤");
        // viewer 注入到位（未替换的占位符会让 PG 报列不存在）
        for st in ["seeding", "leeching", "completed", "incomplete"] {
            assert!(
                !status_pred(Some(st), "7").contains("{viewer}"),
                "{st} 占位符未替换"
            );
        }
    }

    #[test]
    fn bookmark_pred_hits_viewer() {
        let b = bookmark_pred("9");
        assert!(b.contains("bookmarks b") && b.contains("b.user_id = 9"));
    }
}

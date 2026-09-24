//! 楼中楼（0163）：楼层行结构 + 按 root_id 归组。
//! 从 post_read.rs 拆出（post_read 超行数门禁 300）。

use super::*;

/// 楼层行（含 2026-09-23 的作者公开信息与 0163 的楼中楼字段）。
/// `hidden` 不序列化：受保护版块的正文在 Rust 侧替换为「……」后直接下发。
#[derive(sqlx::FromRow, serde::Serialize)]
pub(crate) struct PostRow {
    pub(crate) id: i64,
    pub(crate) username: Option<String>,
    pub(crate) user_id: Option<i64>,
    pub(crate) body: String,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) edited_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) edited_by: Option<i64>,
    /// 点赞数（0116）
    #[sqlx(default)]
    pub(crate) likes: i64,
    /// 当前用户是否已点赞（0116）
    #[sqlx(default)]
    pub(crate) liked_by_me: bool,
    /// 打赏总额（0127）：该楼收到的魔力总和（次数用 tips 单独计数）
    #[sqlx(default)]
    pub(crate) tips: i64,
    /// 打赏次数（0127）
    #[sqlx(default)]
    pub(crate) tip_count: i64,
    /// 楼层作者公开信息（2026-09-23）：头像/等级/入站时间/佩戴勋章（匿名楼为 NULL）
    #[sqlx(default)]
    pub(crate) avatar_url: Option<String>,
    #[sqlx(default)]
    pub(crate) class_name: Option<String>,
    #[sqlx(default)]
    pub(crate) author_joined_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub(crate) worn_medals: Option<serde_json::Value>,
    /// 楼中楼（0163）：所属顶层楼 id（顶层楼自身为 NULL）
    #[sqlx(default)]
    pub(crate) root_id: Option<i64>,
    /// 直接回复的目标楼层 id（顶层楼为 NULL）
    #[sqlx(default)]
    pub(crate) parent_id: Option<i64>,
    /// 被回复楼层作者名（渲染「回复 @xxx」；目标楼不存在/匿名时为 NULL）
    #[sqlx(default)]
    pub(crate) reply_to_name: Option<String>,
    #[serde(skip)]
    pub(crate) hidden: bool,
}

/// 响应行：顶层楼 + 其楼中楼（按 root_id 归组，组内保持时间正序）
#[derive(serde::Serialize)]
pub(crate) struct PostWithReplies {
    #[serde(flatten)]
    pub(crate) post: PostRow,
    pub(crate) replies: Vec<PostRow>,
}

/// 顶层楼保持时间升序；楼中楼按所属顶层楼归组（组内保持升序）。
pub(crate) fn group_by_root(posts: Vec<PostRow>) -> Vec<PostWithReplies> {
    let mut tops: Vec<PostRow> = Vec::new();
    let mut by_root: std::collections::HashMap<i64, Vec<PostRow>> =
        std::collections::HashMap::new();
    for row in posts {
        match row.root_id {
            Some(r) => by_root.entry(r).or_default().push(row),
            None => tops.push(row),
        }
    }
    tops.into_iter()
        .map(|t| {
            let replies = by_root.remove(&t.id).unwrap_or_default();
            PostWithReplies { post: t, replies }
        })
        .collect()
}

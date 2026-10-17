//! 公开主页查询 helpers（从 profile.rs 拆出，300 行门禁）。

/// 近期回帖的取数行型：(post_id, topic_id, 话题标题, forum_id, 版块名,
/// 回复数, 发帖时间)。出参是 JSON，行型只在这一处用，得显式写出来。
type PostRow = (
    i64,
    i64,
    String,
    i64,
    String,
    i64,
    chrono::DateTime<chrono::Utc>,
);

/// 字幕身份查询（0149）：gold 优先于 certified；revoked 行不计
pub(super) async fn subtitle_cert_tier(
    db: &sqlx::PgPool,
    uid: i64,
) -> Option<String> {
    sqlx::query_scalar(
        "SELECT tier FROM user_subtitle_certs WHERE user_id = $1 AND \
         revoked_at IS NULL ORDER BY CASE tier WHEN 'gold' THEN 0 ELSE 1 END \
         LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(db)
    .await
    .unwrap_or(None)
}

/// 近期论坛回帖（主页动态块），出口直接给语义字段名。
///
/// ⚠️ 三条硬口径：
/// 1. **必须带读权限谓词** `f.minclassread <= 读者等级`。旧版只按
///    `p.user_id` 过滤，任何人打开某人的主页就能看到他在隐藏版块
///    里的回帖正文——那是跨权限档的内容泄漏，不是「动态展示」。
/// 2. 楼主帖不算「回帖」（`p.id > 该主题最小帖 id`），与论坛各处
///    `replies = count-1` 同一口径。
/// 3. 旧版把 `[pid, topicId, body, at]` 四元组原样序列化出去，前端只能
///    按下标取值，加一列就得改所有消费方；样图那行要的是「话题标题 +
///    版块 + 回复数」。取数与出参合在同一个函数里也是有意的——本文件在
///    H 层，数据库引用计数按基线只许瘦不许胖。
///
/// 失败容忍 → 空表（主页不该因为一块动态取不到就整页 500）。
pub(super) async fn recent_posts_of(
    db: &sqlx::PgPool,
    uid: i64,
    viewer_class: i32,
) -> Vec<serde_json::Value> {
    sqlx::query_as::<_, PostRow>(
        "SELECT p.id, p.topic_id, t.title, t.forum_id, f.name, \
         greatest((SELECT count(*) FROM posts p2 \
                    WHERE p2.topic_id = t.id) - 1, 0), p.created_at \
         FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         JOIN forums f ON f.id = t.forum_id \
         WHERE p.user_id = $1 AND p.body_text <> '' \
           AND f.minclassread <= $2 \
           AND p.id > (SELECT min(p3.id) FROM posts p3 \
                        WHERE p3.topic_id = p.topic_id) \
         ORDER BY p.id DESC LIMIT 5",
    )
    .bind(uid)
    .bind(viewer_class)
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(
        |(id, topic_id, topic_title, forum_id, forum_name, replies, at)| {
            serde_json::json!({
                "id": id,
                "topic_id": topic_id,
                "topic_title": topic_title,
                "forum_id": forum_id,
                "forum_name": forum_name,
                "replies": replies,
                "created_at": at.to_rfc3339(),
            })
        },
    )
    .collect()
}

//! 公开主页查询 helpers（从 profile.rs 拆出，300 行门禁）。


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

/// 近期论坛回帖（主页动态块；失败容忍 → 空表）
pub(super) async fn recent_posts_of(
    db: &sqlx::PgPool,
    uid: i64,
) -> Vec<(i64, i64, Option<String>, chrono::DateTime<chrono::Utc>)> {
    sqlx::query_as(
        "SELECT p.id, p.topic_id, left(p.body_text, 80), \
         p.created_at FROM posts p \
         WHERE p.user_id = $1 AND p.body_text <> '' \
         ORDER BY p.id DESC LIMIT 5",
    )
    .bind(uid)
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

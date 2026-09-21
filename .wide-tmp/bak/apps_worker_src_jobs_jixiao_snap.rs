//! 绩效期末快照滚动。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 为指定期落全站活跃用户基线快照（幂等：PK 冲突跳过——首个到达的快照即基线，
/// 重跑不覆盖：期初值必须固定，中途覆盖会让 delta 口径漂移）。
pub(crate) async fn jixiao_rollover_snapshot(
    db: &PgPool,
    period: &str,
) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO jixiao_baseline_snapshots (user_id, period, seed_seconds, uploaded, uploads)
        SELECT u.id, $1,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               u.uploaded,
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM users u
        WHERE u.status < 2
          AND (EXISTS(SELECT 1 FROM snatches s2 WHERE s2.user_id = u.id)
               OR u.uploaded > 0)
        ON CONFLICT (user_id, period) DO NOTHING
        "#,
    )
    .bind(period)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

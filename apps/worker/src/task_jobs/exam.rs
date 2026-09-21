//! 考核引擎（0093，tasks 单表方案）：自动派发考核与派发通知。

use sqlx::PgPool;

/// 自动派发考核（worker 分钟级调用，幂等：task_claims UNIQUE(task_id,user_id) 挡重复）。
/// 派发口径：
///   onboard  —— 注册 ≤ exam_onboard_days（默认 30）天、等级 ≥ target_class 的用户
///   periodic —— 等级 ≥ target_class 的全体用户（period 重派：上一个周期认领已结算才再插）
/// 派发即发 PM 通知（P1-4：静默派发用户不知情）。复用 claim 的基线快照列。
pub async fn exam_assign(db: &PgPool) -> anyhow::Result<u64> {
    let onboard_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name='exam_onboard_days'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30);

    let mut tx = db.begin().await?;
    let mut total = 0usize;

    // onboard：新人窗口内 + 未删号 + 未领过（含已结算——转正考核一次性）
    let onboard = sqlx::query(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT t.id, u.id, u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM tasks t
        JOIN users u ON u.class_id >= t.target_class
             AND u.created_at > now() - ($1 || ' days')::interval
        WHERE t.auto_assign AND t.kind = 'onboard'
          AND now() BETWEEN t.starts_at AND t.ends_at
          AND NOT EXISTS (SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = u.id)
        "#,
    )
    .bind(onboard_days.to_string())
    .execute(&mut *tx)
    .await?;
    total += onboard.rows_affected() as usize;

    // periodic：等级达标全体；period='once' 仅未领过；monthly/quarterly 上轮已结算可重派
    let periodic = sqlx::query(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT t.id, u.id, u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM tasks t
        JOIN users u ON u.class_id >= t.target_class
        WHERE t.auto_assign AND t.kind = 'periodic'
          AND now() BETWEEN t.starts_at AND t.ends_at
          AND NOT EXISTS (
                SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = u.id
                  AND (t.period = 'once' OR tc.status = 0))
        "#,
    )
    .execute(&mut *tx)
    .await?;
    total += periodic.rows_affected() as usize;

    // 派发 PM：本轮新插的考核认领（以 claim 无消息为界，避免重跑重复轰炸）
    sqlx::query(
        r#"
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, c.user_id, '考核已派发',
               '您已被派发考核「' || t.name || '」，达成目标即可获得奖励，详情请在「考核&任务」页面查看。'
        FROM task_claims c
        JOIN tasks t ON t.id = c.task_id
        WHERE t.kind IN ('onboard', 'periodic')
          AND c.pm_sent = FALSE
        "#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE task_claims c SET pm_sent = TRUE \
                 FROM tasks t WHERE t.id = c.task_id AND t.kind IN ('onboard','periodic') AND c.pm_sent = FALSE")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    if total > 0 {
        tracing::info!(total, "exam assigned");
    }
    Ok(total as u64)
}

//! 任务结算的达标/失败结算：发奖、扣罚金与站内信通知。

use super::settle::OpenClaim;
use sqlx::PgPool;

/// 达标：status=1 + 发奖（幂等键 task_settle:{claim_id}，防 worker 并发双发）
pub(crate) async fn settle_complete(
    db: &PgPool,
    c: &OpenClaim,
) -> anyhow::Result<bool> {
    let mut tx = db.begin().await?;
    let updated = sqlx::query(
        "UPDATE task_claims SET status = 1, settled_at = now(), reward_paid = $2 \
         WHERE id = $1 AND status = 0",
    )
    .bind(c.id)
    .bind(c.reward)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    if c.reward > 0 {
        let idem = format!("task_settle:{}", c.id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(c.user_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'task_reward', 'task', $3, $4, $5)",
            )
            .bind(c.user_id)
            .bind(c.reward)
            .bind(c.task_id)
            .bind(&idem)
            .bind(balance + c.reward)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
                .bind(c.user_id)
                .bind(c.reward)
                .execute(&mut *tx)
                .await?;
        }
    }
    // 完成通知：转正考核（onboard）用专有文案——「转正」语义不落等级
    // （等级归 class_auto_adjust 管，P1-3 定案：考核通过只做确认 + 发奖，不动 class_id）
    let (subject, body) = if c.kind == "onboard" {
        (
            "转正考核通过",
            format!(
                "恭喜！您已完成新人转正考核「{}」，正式成为本站的一员。奖励 {} 魔力已发放到您的账户。",
                c.task_name, c.reward
            ),
        )
    } else {
        (
            "任务完成通知",
            format!(
                "恭喜！您认领的任务「{}」已完成，奖励 {} 魔力已发放到您的账户。",
                c.task_name, c.reward
            ),
        )
    };
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind(subject)
    .bind(body)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// 失败/超时：status=2；配置了罚金则扣（幂等键 task_penalty:{claim_id}）
pub(crate) async fn settle_fail(
    db: &PgPool,
    c: &OpenClaim,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // 罚金说明文案按实扣额生成（见下方审计修复注释）；未配置罚金时为空串
    let mut penalty_note = String::new();
    let updated = sqlx::query(
        "UPDATE task_claims SET status = 2, settled_at = now() WHERE id = $1 AND status = 0",
    )
    .bind(c.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(());
    }
    if c.penalty > 0 {
        let idem = format!("task_penalty:{}", c.id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(c.user_id)
            .fetch_one(&mut *tx)
            .await?;
            // 罚金只扣到 0，不制造负余额
            let take = balance.min(c.penalty).max(0);
            if take > 0 {
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'task_penalty', 'task', $3, $4, $5)",
                )
                .bind(c.user_id)
                .bind(-take)
                .bind(c.task_id)
                .bind(&idem)
                .bind(balance - take)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1")
                    .bind(c.user_id)
                    .bind(take)
                    .execute(&mut *tx)
                    .await?;
            }
            // 审计修复（文案失实）：扣款额按实（take = min(余额, 罚金)）告知；
            // 旧文案固定写「扣除罚金 {penalty}」，余额不足被部分扣/零扣时与流水对不上。
            penalty_note = if take < c.penalty {
                format!(
                    "，扣除罚金 {} 魔力（余额不足，本次仅能扣到 0，未扣足 {} 魔力）",
                    take,
                    c.penalty - take
                )
            } else {
                format!("，扣除罚金 {} 魔力", take)
            };
        }
    }
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind("任务超时通知")
    .bind(format!(
        "您认领的任务已超出完成时限（{} 天），任务标记为失败{}。",
        c.duration_days, penalty_note
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

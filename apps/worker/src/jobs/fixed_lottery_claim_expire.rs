//! 固定抽签·自定义奖领取超期日扫（从 fixed_lottery.rs 拆出守 300 行门禁）。
//! 与 api/services/fixed_lottery/claim.rs 同口径，改动须双向同步。

use sqlx::PgPool;

/// 自定义奖超期失效；已通过审核永久豁免。返回翻转条数。
pub(crate) async fn fixed_lottery_claim_expire(
    db: &PgPool,
) -> anyhow::Result<u64> {
    // 自定义奖履约终态是 satisfied（不是 fulfilled），旧 SQL 永远扫不到。
    let n = sqlx::query(
        "WITH expired AS ( \
            SELECT w.id FROM fixed_lottery_winners w \
            JOIN fixed_lottery_prizes p ON p.id = w.prize_id \
            WHERE p.reward_kind = 'custom' \
              AND w.claim_deadline_at IS NOT NULL \
              AND w.claim_deadline_at < now() \
              AND w.status IN ('satisfied', 'fulfilled', 'claimed') \
              AND NOT EXISTS (SELECT 1 FROM fixed_lottery_claims c \
                   WHERE c.winner_id = w.id \
                     AND c.status IN ('approved', 'pending_review')) \
         ) \
         UPDATE fixed_lottery_winners SET status = 'failed' \
         WHERE id IN (SELECT id FROM expired)",
    )
    .execute(db)
    .await?
    .rows_affected();
    sqlx::query(
        "UPDATE fixed_lottery_claims c SET status = 'expired', \
             updated_at = now() \
         FROM fixed_lottery_winners w \
         WHERE c.winner_id = w.id AND w.status = 'failed' \
           AND c.status IN ('pending_review', 'rejected') \
           AND w.claim_deadline_at IS NOT NULL \
           AND w.claim_deadline_at < now()",
    )
    .execute(db)
    .await?;
    Ok(n)
}

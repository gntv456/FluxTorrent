//! P2 批三 job（0228）：求种过期回收（悬赏退还发起人）。
//! 对齐 UNIT3D AutoRecycleClaimedTorrentRequests 的「无人响应自动关闭」语义；
//! 我们的求种无 claim 环节，过期口径 = `request_expire_days` 天内未被应种。
//! 记账口径抄 bank_jobs/deposit.rs（行锁→幂等查→入账，worker 侧自写 SQL）。

use sqlx::PgPool;

pub(crate) async fn request_expire(db: &PgPool) -> anyhow::Result<u64> {
    let days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'request_expire_days')::bigint, 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    if days <= 0 {
        return Ok(0);
    }
    // 过期集合：status=0 且发布超 N 天。逐行退还悬赏（幂等键
    // req-refund:{id} 防双退），再置 status=2（过期）。
    let expired: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, bounty FROM requests \
         WHERE status = 0 AND created_at < now() - make_interval(days => $1)",
    )
    .bind(days)
    .fetch_all(db)
    .await?;
    for (id, requester, bounty) in &expired {
        if *bounty > 0 {
            let idem = format!("req-refund:{id}");
            let mut tx = db.begin().await?;
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $1)",
            )
            .bind(&idem)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                let balance: i64 = sqlx::query_scalar(
                    "SELECT spark_balance FROM users WHERE id = $1 \
                     FOR UPDATE",
                )
                .bind(requester)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, \
                     kind, ref_type, ref_id, idempotency_key, \
                     balance_after) VALUES (nextval('spark_ledger_id_seq'), \
                     $1, $2, 'request_refund', 'request', $3, $4, $5)",
                )
                .bind(requester)
                .bind(bounty)
                .bind(id)
                .bind(&idem)
                .bind(balance + bounty)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE users SET spark_balance = $2 WHERE id = $1",
                )
                .bind(requester)
                .bind(balance + bounty)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
        sqlx::query("UPDATE requests SET status = 2 WHERE id = $1")
            .bind(id)
            .execute(db)
            .await?;
    }
    if !expired.is_empty() {
        tracing::info!(n = expired.len(), "expired requests recycled");
    }
    Ok(expired.len() as u64)
}

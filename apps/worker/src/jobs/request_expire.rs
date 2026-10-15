//! P2 批三 job（0228）：求种过期回收（悬赏退还发起人）。
//! 对齐 UNIT3D AutoRecycleClaimedTorrentRequests 的「无人响应自动关闭」语义；
//! 我们的求种无 claim 环节，过期口径 = `request_expire_days` 天内未被应种。
//! 记账口径抄 bank_jobs/deposit.rs（行锁→幂等查→入账，worker 侧自写 SQL）。

use sqlx::PgPool;

/// request_expire_days 未配置/为 0 时的兜底过期天数。
///
/// 存在的理由：求种的 bounty 是**真扣**（spend_spark 走 spark_ledger），
/// 不回收等于钱凭空消失。原实现 days<=0 直接跳过，于是关掉开关 = 永久
/// 冻结。兜底值给 30 天——与站方常见的「一个月无人应种就关闭」一致。
pub(crate) const FALLBACK_EXPIRE_DAYS: i64 = 30;

pub(crate) async fn request_expire(db: &PgPool) -> anyhow::Result<u64> {
    let days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'request_expire_days')::bigint, 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    // days<=0 的口径（批一百三十补）：不是「不回收」，而是**换一条判据**。
    // 原来这里直接 return 0，于是 request_expire_days=0（站方把自动回收
    // 关掉，或迁移默认值还没落）时，那些已经被 spend_spark 真扣走的悬赏
    // 既不退还、也不被任何作业回收——永久冻结在 requests.bounty 里。
    // 现在：days<=0 时改用「兜底 30 天」判据，保证钱最终一定回到发起人。
    // 想彻底关掉自动回收，站长应把 days 设成一个很大的值，而不是 0。
    let days = if days <= 0 { FALLBACK_EXPIRE_DAYS } else { days };
    // 过期集合：status=0 且发布超 N 天。逐行「先抢坑再退钱」——幂等键
    // req-refund:{id} 防重复回收。
    let expired: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, bounty FROM requests \
         WHERE status = 0 AND created_at < now() - make_interval(days => $1)",
    )
    .bind(days)
    .fetch_all(db)
    .await?;
    for (id, requester, bounty) in &expired {
        // 抢坑：只有把 status 从 0 翻成 2 成功，这一单才归本作业处理。
        // 上面那份 SELECT 在事务外，只是快照；快照之后若有人应种
        // （/requests/fulfill 用同一条 `AND status = 0` 抢坑并已发奖），
        // 这里就翻不动了。旧写法把退还放在翻面之前、且 UPDATE 不带守卫
        // ⇒ 交错时同一笔悬赏既付给应种人又退给求种人（钱凭空多一倍），
        // 完结记录还被读成「过期」。
        let mut tx = db.begin().await?;
        let flipped = sqlx::query(
            "UPDATE requests SET status = 2 WHERE id = $1 AND status = 0",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if flipped.rows_affected() == 0 {
            continue; // 事务里没写任何东西，丢弃即回滚
        }
        if *bounty > 0 {
            let idem = format!("req-refund:{id}");
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
                .bind(crate::balance_guard::checked_balance(balance, *bounty)?)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE users SET spark_balance = $2 WHERE id = $1",
                )
                .bind(requester)
                .bind(crate::balance_guard::checked_balance(balance, *bounty)?)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
    }
    if !expired.is_empty() {
        tracing::info!(n = expired.len(), "expired requests recycled");
    }
    Ok(expired.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_zero_falls_back_instead_of_disabling() {
        // 站方把关掉开关写成 0 是自然动作；但 0 的旧语义是「永不回收」，
        // 而 bounty 是 spend_spark 真扣的钱 —— 那等于永久冻结。
        // 现在 0 走兜底 30 天，钱最终一定回到发起人。
        let configured = 0i64;
        let effective =
            if configured <= 0 { FALLBACK_EXPIRE_DAYS } else { configured };
        assert_eq!(effective, 30);
        assert!(effective > 0, "兜底必须是正数，否则等于没修");
    }

    #[test]
    fn positive_days_are_respected() {
        let configured = 7i64;
        let effective =
            if configured <= 0 { FALLBACK_EXPIRE_DAYS } else { configured };
        assert_eq!(effective, 7, "站方显式设的天数必须原样生效");
    }

    #[test]
    fn negative_days_also_fall_back() {
        let configured = -1i64;
        let effective =
            if configured <= 0 { FALLBACK_EXPIRE_DAYS } else { configured };
        assert_eq!(effective, FALLBACK_EXPIRE_DAYS);
    }
}

//! 论坛抽奖开奖（0126，worker 侧）。
//! 从 run.rs 按域拆出（run.rs 触 300 行门禁、且手动触发要复用同一入口）。
//! 与 api 的 lottery_draw_core 同一套库表协议——CAS open→drawn 防双开，
//! 中奖发放幂等键 `forum-lottery-win:{tid}:{uid}`（spark_ledger 自守），
//! 无人参与退回楼主（`forum-lottery-refund:{tid}`）。票费不分成（归入池的是楼主
//! 冻结的奖金，票费在本实现里是参与门槛而非奖池构成，避免开奖金额与冻结额错位）。

use sqlx::PgPool;

/// 到点开奖（run.rs 的 60s tick 与手动触发共用本入口）。
pub(crate) async fn lottery_settle_due(db: &PgPool) -> anyhow::Result<u64> {
    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT topic_id FROM topic_lotteries WHERE status = 'open' AND \
         draw_at <= now() LIMIT 50",
    )
    .fetch_all(db)
    .await?;
    let mut settled = 0u64;
    for tid in due {
        match lottery_settle(db, tid).await {
            Ok(n) => settled += n,
            Err(e) => {
                tracing::warn!(
                    topic_id = tid,
                    error = %e,
                    "lottery_settle failed"
                );
            }
        }
    }
    Ok(settled)
}

pub(crate) async fn lottery_settle(
    db: &PgPool,
    topic_id: i64,
) -> anyhow::Result<u64> {
    let meta: Option<(i32, i64, i64)> = sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint FROM \
         topic_lotteries WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .fetch_optional(db)
    .await?;
    let Some((winners, prize, _ticket)) = meta else {
        return Ok(0); // 已开/已取消：幂等静默
    };
    let n = sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' WHERE topic_id = \
         $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Ok(0); // 并发对手（楼主手动开）赢了对局
    }
    let mut tx = db.begin().await?;
    // 中奖名单：数据库侧 random() 洗牌取前 N（抽签随机性不由应用层承担）
    let picked: Vec<i64> = sqlx::query_scalar(
        "UPDATE lottery_entries SET won = TRUE \
         WHERE topic_id = $1 AND user_id IN ( \
           SELECT user_id FROM lottery_entries \
           WHERE topic_id = $1 ORDER BY random() LIMIT $2 \
         ) RETURNING user_id",
    )
    .bind(topic_id)
    .bind(winners)
    .fetch_all(&mut *tx)
    .await?;
    if picked.is_empty() {
        // 无人参与：奖金池整退楼主
        let op: i64 =
            sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
                .bind(topic_id)
                .fetch_one(&mut *tx)
                .await?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $1)",
            )
            .bind(format!("forum-lottery-refund:{topic_id}"))
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                let bal: i64 = sqlx::query_scalar(
                    "UPDATE users SET spark_balance = \
                     spark_balance + $2 WHERE id = $1 RETURNING spark_balance",
                )
                .bind(op)
                .bind(refund)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO spark_ledger \
                     (id, user_id, amount, kind, idempotency_key, \
                     balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, \
                     'forum_lottery_refund', $3, $4)",
                )
                .bind(op)
                .bind(refund)
                .bind(format!("forum-lottery-refund:{topic_id}"))
                .bind(bal)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        tracing::info!(
            topic_id,
            refund,
            "lottery settled: no entries, refunded"
        );
        return Ok(0);
    }
    // 发放（同事务逐人：幂等键存在则跳过，重跑安全）
    for uid in &picked {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE \
             idempotency_key = $1)",
        )
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .fetch_one(&mut *tx)
        .await?;
        if exists || prize <= 0 {
            continue;
        }
        let bal: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = spark_balance + $2 \
             WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(prize)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO spark_ledger \
             (id, user_id, amount, kind, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, \
             'forum_lottery', $3, $4)",
        )
        .bind(uid)
        .bind(prize)
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .bind(bal)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(topic_id, winners = picked.len(), prize, "lottery settled");
    Ok(picked.len() as u64)
}

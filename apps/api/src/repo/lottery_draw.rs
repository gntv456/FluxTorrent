//! 抽奖开奖的 SQL 唯一居所（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`community_http/lottery_draw.rs`（手动开奖 + worker 到点 tick
//! 共用同一入口）。CAS `open→drawn` 是防重复发奖的关键，必须在同一
//! 事务里与发奖一起提交——旧版先 autocommit 置位再开事务，中途崩溃会
//! 让状态已 drawn 而奖金永久卡死（worker 三轮审计 P2 的原案）。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 抽奖元信息：(winners, prize_per_winner, ticket_spark, status)。
pub type LotteryMeta = (i32, i64, i64, String);

/// 读抽奖元信息。
pub async fn meta(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Option<LotteryMeta>> {
    Ok(sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint, \
         status FROM topic_lotteries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// CAS 置 drawn：受影响 0 行 = 对手（楼主手动开）已经赢了对局。
pub async fn cas_drawn(repo: &Repo, topic_id: i64) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' \
         WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 全部参与者 id（应用层洗牌用；数量级 ≤ 数百）。
pub async fn participant_ids(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT user_id FROM lottery_entries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 楼主（退款收件人）。
pub async fn topic_owner(repo: &Repo, topic_id: i64) -> DomainResult<i64> {
    sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
        .bind(topic_id)
        .fetch_one(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

/// 标记中奖。
pub async fn mark_won(
    repo: &Repo,
    topic_id: i64,
    user_id: i64,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE lottery_entries SET won = TRUE \
         WHERE topic_id = $1 AND user_id = $2",
    )
    .bind(topic_id)
    .bind(user_id)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 把全部参与者标记为未中奖（结算开始时清一次，保证重跑口径一致）。
pub async fn reset_won(repo: &Repo, topic_id: i64) -> DomainResult<()> {
    sqlx::query("UPDATE lottery_entries SET won = FALSE WHERE topic_id = $1")
        .bind(topic_id)
        .execute(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

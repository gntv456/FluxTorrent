//! 删主题（DELETE /forums/topics/{id}）的 SQL 唯一居所。
//!
//! 2026-10-16 下沉：见 follows.rs 头注。删主题要回收三类钱（悬赏退回、
//! 抽奖退回、发帖/回帖火花回收），各笔 earn_spark 在 H 层按幂等键发，
//! 这里只负责「读现状」和「改状态」，金额判定的 SQL 全在本文件。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 主题所属版块（None = 主题不存在）。
pub async fn topic_forum_id(
    repo: &Repo,
    tid: i64,
) -> DomainResult<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_optional(&repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?,
    )
}

/// 主题楼主。
pub async fn topic_owner(repo: &Repo, tid: i64) -> DomainResult<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_one(&repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?,
    )
}

/// 待退的悬赏金额（0124：只有 open 且 > 0 的才退；已 awarded 的不动）。
pub async fn open_bounty(repo: &Repo, tid: i64) -> DomainResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT bounty_spark FROM topics \
         WHERE id = $1 AND topic_type = 'bounty' \
         AND bounty_status = 'open' AND bounty_spark > 0",
    )
    .bind(tid)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 把悬赏标成已退（幂等：只有 open 才改，affected=0 说明已处理过）。
pub async fn mark_bounty_refunded(repo: &Repo, tid: i64) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE topics SET bounty_status = 'refunded' \
         WHERE id = $1 AND bounty_status = 'open'",
    )
    .bind(tid)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 待退的抽奖奖金池（0126：winners × prize_per_winner）。
///
/// 票价不退——参与者享受了参与过程；与删悬赏帖不追回已发赏金同一
/// 不对称口径。
pub async fn open_lottery_pool(
    repo: &Repo,
    tid: i64,
) -> DomainResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT winners::bigint * prize_per_winner FROM topic_lotteries \
         WHERE topic_id = $1 AND status = 'open' AND prize_per_winner > 0",
    )
    .bind(tid)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 把抽奖标成已取消（幂等：只有 open 才改）。
pub async fn mark_lottery_cancelled(
    repo: &Repo,
    tid: i64,
) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE topic_lotteries SET status = \
         'cancelled' WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(tid)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 待收回的回帖奖励额（按人汇总；审计 P1：排除楼主首帖）。
///
/// 主题删除会 CASCADE 掉全部 posts，逐笔回收回帖奖励保持对称。旧版把
/// 楼主首帖也计为「回帖」再 -1，楼主实扣 -3（发帖 -2 + 首帖按回帖 -1），
/// 这里按「非楼主 或 非首帖」把首帖排除，楼主只按发帖口径 -2。
///
/// P-456：返回的不再是「回帖条数」而是「这些人当初真拿到了多少火花」——
/// 回帖奖励的额度改由面板 `makepost` 决定后，条数 × 常数就不等于发放额了
/// （站长先按 1 发帖、再把面板调成 3，删帖会多扣两倍）。发放侧写的幂等键
/// `forum-reply:{uid}:{pid}` 可直接回查 `spark_ledger`。
pub async fn reply_reward_debt(
    repo: &Repo,
    tid: i64,
    op: i64,
) -> Vec<(i64, i64)> {
    sqlx::query_as(
        "SELECT p.user_id, COALESCE(sum(l.amount), 0)::bigint \
         FROM posts p \
         LEFT JOIN spark_ledger l \
           ON l.idempotency_key = 'forum-reply:' || p.user_id::text \
              || ':' || p.id::text \
         WHERE p.topic_id = $1 AND NOT (p.user_id = $2 \
           AND p.id = (SELECT min(x.id) FROM posts x \
                        WHERE x.topic_id = $1)) \
         GROUP BY p.user_id",
    )
    .bind(tid)
    .bind(op)
    .fetch_all(&repo.db)
    .await
    .unwrap_or_default()
}

/// 删主题本体（回帖 / 投票 / 悬赏由外键 CASCADE）。
pub async fn delete_topic(repo: &Repo, tid: i64) -> DomainResult<()> {
    sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(tid)
        .execute(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

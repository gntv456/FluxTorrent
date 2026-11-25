//! 固定抽签·管理口读侧（0520 拆件守 300 行门禁）：全量列表/CAS 发布/
//! CAS 取消/重试清零/领取队列/中奖履约状态/创建事务落库。
//! 用户口读侧在 fixed_lottery.rs；worker 侧镜像见 worker/jobs。

use super::Repo;
use crate::errors::{DomainError, DomainResult};
use serde_json::Value;

/// 管理口·活动全量表行。
pub type AdminListRow = (i64, String, String, String, String, i64);

pub async fn admin_list_all(repo: &Repo) -> DomainResult<Vec<AdminListRow>> {
    Ok(sqlx::query_as(
        "SELECT id, participation_code, title, status, trigger_type, \
                ticket_price \
         FROM fixed_lottery_campaigns \
         ORDER BY (status = 'open') DESC, id DESC LIMIT 200",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// CAS 发布：0 行 = 不在草稿态。
pub async fn cas_publish(repo: &Repo, cid: i64) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE fixed_lottery_campaigns SET status = 'open' \
         WHERE id = $1 AND status = 'draft'",
    )
    .bind(cid)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// CAS 取消：0 行 = 状态不允许。
pub async fn cas_cancel(
    repo: &Repo,
    cid: i64,
    reason: &str,
) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE fixed_lottery_campaigns \
         SET status = 'cancelled', cancel_reason = $2, \
             cancelled_at = now() \
         WHERE id = $1 AND status IN ('draft', 'open')",
    )
    .bind(cid)
    .bind(reason)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 重试口：退避计时清零（exhausted 回 failed）。
pub async fn reset_retry(repo: &Repo, winner_id: i64) -> DomainResult<()> {
    sqlx::query(
        "UPDATE fixed_lottery_fulfillments \
         SET status = CASE WHEN status = 'exhausted' THEN 'failed' \
                           ELSE status END, updated_at = now() \
         WHERE winner_id = $1",
    )
    .bind(winner_id)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 管理口·待审领取队列行。
pub type ClaimQueueRow = (i64, String, Value, String);

pub async fn claim_queue(repo: &Repo) -> DomainResult<Vec<ClaimQueueRow>> {
    Ok(sqlx::query_as(
        "SELECT w.id, w.username_snapshot, c.submission, \
                w.prize_name_snapshot \
         FROM fixed_lottery_claims c \
         JOIN fixed_lottery_winners w ON w.id = c.winner_id \
         WHERE c.status = 'pending_review' \
         ORDER BY c.submitted_at DESC LIMIT 200",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 管理口·中奖与履约状态行。
pub type AdminWinnerRow = (
    i64,
    i64,
    String,
    String,
    i32,
    i32,
    String,
    Option<String>,
    Option<i32>,
    Option<String>,
    Option<chrono::DateTime<chrono::Utc>>,
);

pub async fn admin_winners_of(
    repo: &Repo,
    cid: i64,
) -> DomainResult<Vec<AdminWinnerRow>> {
    Ok(sqlx::query_as(
        "SELECT w.id, w.user_id, w.username_snapshot, \
                w.prize_name_snapshot, w.slot_no, w.draw_position, \
                w.status, f.status, f.attempt_count, f.last_error, \
                f.next_retry_at \
         FROM fixed_lottery_winners w \
         LEFT JOIN fixed_lottery_fulfillments f ON f.winner_id = w.id \
         WHERE w.campaign_id = $1 ORDER BY w.draw_position",
    )
    .bind(cid)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 参与码重复检测（创建口）。
pub async fn code_exists(repo: &Repo, code: &str) -> DomainResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fixed_lottery_campaigns \
         WHERE participation_code = $1)",
    )
    .bind(code)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}
/// 创建口·事务内插入活动与奖项（write http 用例的落库半场）。
pub async fn create_campaign_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: &str,
    title: &str,
    intro: &str,
    trigger_type: &str,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    scheduled_draw_at: Option<chrono::DateTime<chrono::Utc>>,
    threshold_count: Option<i32>,
    fallback_draw_at: Option<chrono::DateTime<chrono::Utc>>,
    ticket_price: i64,
    eligibility: &Value,
    claim_default_days: Option<i32>,
    created_by: i64,
    prizes: &[(String, i32, String, Value)],
) -> DomainResult<i64> {
    let cid: i64 = sqlx::query_scalar(
        "INSERT INTO fixed_lottery_campaigns \
             (participation_code, title, intro, trigger_type, starts_at, \
              scheduled_draw_at, threshold_count, fallback_draw_at, \
              ticket_price, eligibility, claim_default_days, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, \
                 COALESCE($11, 14), $12) RETURNING id",
    )
    .bind(code)
    .bind(title)
    .bind(intro)
    .bind(trigger_type)
    .bind(starts_at)
    .bind(scheduled_draw_at)
    .bind(threshold_count)
    .bind(fallback_draw_at)
    .bind(ticket_price)
    .bind(eligibility)
    .bind(claim_default_days)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (i, (name, qty, kind, cfg)) in prizes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO fixed_lottery_prizes \
                 (campaign_id, sort_order, name, quantity, reward_kind, \
                  reward_config) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(cid)
        .bind(i as i32)
        .bind(name)
        .bind(qty)
        .bind(kind)
        .bind(cfg)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(cid)
}

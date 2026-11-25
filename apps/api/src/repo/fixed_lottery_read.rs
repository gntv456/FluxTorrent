//! 固定抽签·用户口读侧（0520 拆件守 300 行门禁）：列表/中奖名单/
//! 奖项配置/本人 winner 定位/用户名快照。
//! 写侧（报名/开奖落子）在 fixed_lottery.rs，管理口读侧在 fixed_lottery_admin.rs。

use super::Repo;
use crate::errors::{DomainError, DomainResult};
use serde_json::Value;

/// 活动列表行（用户口投影）。
pub type ListRow = (
    i64,
    String,
    String,
    String,
    String,
    Option<chrono::DateTime<chrono::Utc>>,
    i64,
    Option<Value>,
);

pub async fn list_open_drawn(repo: &Repo) -> DomainResult<Vec<ListRow>> {
    Ok(sqlx::query_as(
        "SELECT c.id, c.participation_code, c.title, c.status, \
                c.trigger_type, c.scheduled_draw_at, c.ticket_price, \
                c.draw_meta \
         FROM fixed_lottery_campaigns c \
         WHERE c.status IN ('open', 'drawn') \
         ORDER BY (c.status = 'open') DESC, \
                  c.scheduled_draw_at DESC NULLS LAST, c.id DESC \
         LIMIT 50",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 中奖名单行（详情投影；submission 来自领取表 LEFT JOIN）。
pub type WinnerListRow = (
    i64,
    i64,
    String,
    String,
    i32,
    i32,
    String,
    Option<Value>,
    Option<String>,
    Option<String>,
);

pub async fn winners_of(
    repo: &Repo,
    cid: i64,
) -> DomainResult<Vec<WinnerListRow>> {
    Ok(sqlx::query_as(
        "SELECT w.user_id, w.prize_id, w.username_snapshot, \
                w.prize_name_snapshot, w.slot_no, w.draw_position, \
                w.status, c.submission, c.status, c.reject_reason \
         FROM fixed_lottery_winners w \
         LEFT JOIN fixed_lottery_claims c ON c.winner_id = w.id \
         WHERE w.campaign_id = $1 ORDER BY w.draw_position",
    )
    .bind(cid)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 开奖事务内读奖项（与 prizes_of 同口径，连接换成事务句柄）。
pub async fn prizes_of_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
) -> DomainResult<Vec<super::fixed_lottery::PrizeRow>> {
    Ok(sqlx::query_as(
        "SELECT id, sort_order, name, quantity, reward_kind, reward_config \
         FROM fixed_lottery_prizes WHERE campaign_id = $1 \
         ORDER BY sort_order, id",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 开奖事务内读候选（CAS 之后调用；与 candidate_uids 同一把尺）。
pub async fn candidate_uids_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
) -> DomainResult<Vec<i64>> {
    let elig: Option<Value> = sqlx::query_scalar(
        "SELECT eligibility FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let elig = elig.unwrap_or(Value::Null);
    let min = elig.get("min_class").and_then(|v| v.as_i64());
    let max = elig.get("max_class").and_then(|v| v.as_i64());
    let reg = elig.get("registered_days_min").and_then(|v| v.as_i64());
    let excl: Vec<i64> = elig
        .get("exclude_uids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    Ok(sqlx::query_scalar(
        "SELECT p.user_id FROM fixed_lottery_participants p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.campaign_id = $1 \
           AND u.status = 0 \
           AND ($2::bigint IS NULL OR u.class_id >= $2::bigint) \
           AND ($3::bigint IS NULL OR u.class_id <= $3::bigint) \
           AND ($4::bigint IS NULL \
                OR now() - u.created_at >= make_interval(days => $4::int)) \
           AND NOT (u.id = ANY($5::bigint[]))",
    )
    .bind(cid)
    .bind(min)
    .bind(max)
    .bind(reg)
    .bind(&excl)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 本人中奖行 id（领取资料提交需要）。
pub async fn winner_id_of(
    repo: &Repo,
    cid: i64,
    uid: i64,
) -> DomainResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM fixed_lottery_winners \
         WHERE campaign_id = $1 AND user_id = $2",
    )
    .bind(cid)
    .bind(uid)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

pub async fn reward_kind_of(
    repo: &Repo,
    prize_id: i64,
) -> DomainResult<String> {
    Ok(sqlx::query_scalar(
        "SELECT reward_kind FROM fixed_lottery_prizes WHERE id = $1",
    )
    .bind(prize_id)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

pub async fn username_of(repo: &Repo, uid: i64) -> DomainResult<String> {
    Ok(
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_one(&repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?,
    )
}

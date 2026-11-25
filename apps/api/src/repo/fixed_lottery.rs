//! 固定抽签的 SQL 唯一居所（FRAME §2：H 层不下沉 sqlx）。
//!
//! 消费方：`services/fixed_lottery.rs`（报名/开奖/履约用例）与
//! `games_http/fixed_lottery_*.rs`（读侧参数适配）。
//! CAS `open→drawn` 语义同 `repo/lottery_draw.rs`：受影响 0 行 = 对手已赢，
//! 调用方必须让位；开奖写（winner/fulfillment/draw_meta）与 CAS 同事务。

use super::Repo;
use crate::errors::{DomainError, DomainResult};
use serde_json::Value;

pub type DrawMeta = Value;

/// 活动行（services/repo 内部传输；http 侧另行投影）。
#[derive(Debug, sqlx::FromRow)]
pub struct CampaignRow {
    pub id: i64,
    pub participation_code: String,
    pub title: String,
    pub intro: String,
    pub status: String,
    pub trigger_type: String,
    pub starts_at: Option<chrono::DateTime<chrono::Utc>>,
    pub scheduled_draw_at: Option<chrono::DateTime<chrono::Utc>>,
    pub threshold_count: Option<i32>,
    pub fallback_draw_at: Option<chrono::DateTime<chrono::Utc>>,
    pub ticket_price: i64,
    pub eligibility: Value,
    pub claim_default_days: i32,
    pub draw_meta: Option<Value>,
    pub drawn_at: Option<chrono::DateTime<chrono::Utc>>,
    pub drawn_by: Option<i64>,
}

pub async fn campaign_by_code(
    repo: &Repo,
    code: &str,
) -> DomainResult<Option<CampaignRow>> {
    Ok(sqlx::query_as(
        "SELECT id, participation_code, title, intro, status, trigger_type, \
         starts_at, scheduled_draw_at, threshold_count, fallback_draw_at, \
         ticket_price, eligibility, claim_default_days, draw_meta, \
         drawn_at, drawn_by \
         FROM fixed_lottery_campaigns WHERE participation_code = $1",
    )
    .bind(code)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

pub async fn campaign_by_id(
    repo: &Repo,
    id: i64,
) -> DomainResult<Option<CampaignRow>> {
    Ok(sqlx::query_as(
        "SELECT id, participation_code, title, intro, status, trigger_type, \
         starts_at, scheduled_draw_at, threshold_count, fallback_draw_at, \
         ticket_price, eligibility, claim_default_days, draw_meta, \
         drawn_at, drawn_by \
         FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 奖项 (id, sort_order, name, quantity, reward_kind, reward_config)。
pub type PrizeRow = (i64, i32, String, i32, String, Value);

pub async fn prizes_of(repo: &Repo, cid: i64) -> DomainResult<Vec<PrizeRow>> {
    Ok(sqlx::query_as(
        "SELECT id, sort_order, name, quantity, reward_kind, reward_config \
         FROM fixed_lottery_prizes WHERE campaign_id = $1 \
         ORDER BY sort_order, id",
    )
    .bind(cid)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 有效报名 uid（开奖时重查资格：等级区间/注册天数/排除名单）。
///
/// eligibility JSONB 键：min_class/max_class（i64）、registered_days_min
/// （i64）、exclude_uids（数组）。空对象 = 只要求已报名。
pub async fn candidate_uids(repo: &Repo, cid: i64) -> DomainResult<Vec<i64>> {
    let elig: Option<Value> = sqlx::query_scalar(
        "SELECT eligibility FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(cid)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let elig = elig.unwrap_or(Value::Null);
    let min_class = elig.get("min_class").and_then(|v| v.as_i64());
    let max_class = elig.get("max_class").and_then(|v| v.as_i64());
    let reg_days = elig.get("registered_days_min").and_then(|v| v.as_i64());
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
                OR now() - u.created_at \
                   >= make_interval(days => $4::int)) \
           AND NOT (u.id = ANY($5::bigint[]))",
    )
    .bind(cid)
    .bind(min_class)
    .bind(max_class)
    .bind(reg_days)
    .bind(&excl)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// CAS 置 drawn：受影响 0 行 = 已被并发开奖（worker/手动/达标触发）抢先。
pub async fn cas_drawn(repo: &Repo, cid: i64) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE fixed_lottery_campaigns \
         SET status = 'drawn', drawn_at = now() \
         WHERE id = $1 AND status = 'open'",
    )
    .bind(cid)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 开奖事务内的落子三写：winner 批量 + fulfillment 批量 + draw_meta。
/// 调用方（services::draw）已持有 CAS；这里只在给定事务上追加写入。
pub async fn insert_winners_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
    seed_hex: &str,
    rows: &[(i64, i32, i64, i64, String, String)],
    draw_meta: &Value,
) -> DomainResult<()> {
    // rows: (prize_id, slot_no, draw_position, uid, username, prize_name)
    for &(prize_id, slot_no, position, uid, ref uname, ref pname) in rows {
        // selection_key 进库供第三方离线复算比对
        let key =
            crate::domain::fixed_lottery::candidate_key(seed_hex, cid, uid);
        sqlx::query(
            "INSERT INTO fixed_lottery_winners \
             (campaign_id, prize_id, user_id, username_snapshot, \
              prize_name_snapshot, slot_no, draw_position, \
              selection_key, status, claim_deadline_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', \
                 CASE WHEN EXISTS ( \
                      SELECT 1 FROM fixed_lottery_prizes p \
                      WHERE p.id = $2 AND p.reward_kind = 'custom') \
                 THEN now() + make_interval(days => ( \
                      SELECT c.claim_default_days \
                      FROM fixed_lottery_campaigns c WHERE c.id = $1)) \
                 ELSE NULL END) \
             ON CONFLICT (campaign_id, user_id) DO NOTHING",
        )
        .bind(cid)
        .bind(prize_id)
        .bind(uid)
        .bind(uname)
        .bind(pname)
        .bind(slot_no)
        .bind(position)
        .bind(hex_key(&key))
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO fixed_lottery_fulfillments (winner_id, status) \
             SELECT id, 'pending' FROM fixed_lottery_winners \
             WHERE campaign_id = $1 AND user_id = $2 \
             ON CONFLICT (winner_id) DO NOTHING",
        )
        .bind(cid)
        .bind(uid)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "UPDATE fixed_lottery_campaigns SET draw_meta = $2 WHERE id = $1",
    )
    .bind(cid)
    .bind(draw_meta)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

fn hex_key(k: &[u8; 32]) -> String {
    k.iter().map(|x| format!("{x:02x}")).collect()
}

/// 报名：PK 冲突 = 已报名，幂等吞掉。返回是否新报名。
/// 报名：PK 冲突 = 已报名，幂等吞掉。返回是否新报名。
/// 先对活动行 FOR SHARE：与开奖 CAS 的 FOR UPDATE 互斥，
/// 保证「开奖快照读取之后不能再挤进新报名」。
pub async fn join_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
    uid: i64,
    username: &str,
    ip: Option<&str>,
) -> DomainResult<bool> {
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM fixed_lottery_campaigns \
         WHERE id = $1 AND status = 'open' FOR SHARE",
    )
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open.is_none() {
        return Err(DomainError::Validation("活动不在报名期".into()));
    }
    let n = sqlx::query(
        "INSERT INTO fixed_lottery_participants \
         (campaign_id, user_id, username_snapshot, last_ip) \
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(cid)
    .bind(uid)
    .bind(username)
    .bind(ip)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(n > 0)
}

pub async fn joined_uids(repo: &Repo, cid: i64) -> DomainResult<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT user_id FROM fixed_lottery_participants \
         WHERE campaign_id = $1 ORDER BY joined_at, user_id",
    )
    .bind(cid)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

pub async fn participant_count(repo: &Repo, cid: i64) -> DomainResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM fixed_lottery_participants \
         WHERE campaign_id = $1",
    )
    .bind(cid)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

pub async fn has_joined(repo: &Repo, cid: i64, uid: i64) -> DomainResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fixed_lottery_participants \
         WHERE campaign_id = $1 AND user_id = $2)",
    )
    .bind(cid)
    .bind(uid)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 注册时长门槛（报名侧快筛；与 candidate_uids 同一把尺）。
/// 漏判会让未达门槛的人付费报名、开奖时却被静默剔除。
pub async fn registration_ok(
    repo: &Repo,
    uid: i64,
    min_days: i64,
) -> DomainResult<bool> {
    let created: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM users WHERE id = $1")
            .bind(uid)
            .fetch_one(&repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let days = min_days.clamp(0, 36_500);
    Ok(chrono::Utc::now() >= created + chrono::Duration::days(days))
}

//! 自定义奖品领取用例：中奖人提交动态表单 → 管理员批准/驳回 → 超期转 expired。
//!
//! v1 只收文本字段（≤10 个，schema 存 prize.reward_config.fields）；
//! 附件上传与审核留后续批。驳回后可重新提交（状态机回到 pending_review）。

use super::tx_err;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use serde_json::json;

/// 中奖人提交领取资料。驳回后可重交；已 approved 幂等拒绝。
pub(crate) async fn submit_db(
    db: &sqlx::PgPool,
    uid: i64,
    winner_id: i64,
    fields: serde_json::Value,
) -> DomainResult<()> {
    let row: Option<(i64, String, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT w.user_id, p.reward_kind, w.claim_deadline_at \
             FROM fixed_lottery_winners w \
             JOIN fixed_lottery_prizes p ON p.id = w.prize_id \
             WHERE w.id = $1",
        )
        .bind(winner_id)
        .fetch_optional(db)
        .await
        .map_err(tx_err)?;
    let Some((owner, kind, deadline)) = row else {
        return Err(DomainError::NotFound(winner_id));
    };
    if owner != uid {
        return Err(DomainError::Validation(
            "只有中奖人本人可以提交领取资料".into(),
        ));
    }
    if kind != "custom" {
        return Err(DomainError::Validation("该奖品无需领取流程".into()));
    }
    if let Some(d) = deadline {
        if chrono::Utc::now() > d {
            return Err(DomainError::Validation("领取期限已过".into()));
        }
    }
    let cfg: serde_json::Value = sqlx::query_scalar(
        "SELECT p.reward_config FROM fixed_lottery_prizes p \
         JOIN fixed_lottery_winners w ON w.prize_id = p.id \
         WHERE w.id = $1",
    )
    .bind(winner_id)
    .fetch_one(db)
    .await
    .map_err(tx_err)?;
    validate_fields(&cfg, &fields)?;

    // 条件 UPSERT = 库上 CAS：管理员在 SELECT 之后抢先 approve 时，
    // 这次重交必须失败，不能把已通过的记录改回待审。
    let mut tx = db.begin().await.map_err(tx_err)?;
    let n = sqlx::query(
        "INSERT INTO fixed_lottery_claims \
             (winner_id, submission, status, submitted_at) \
         VALUES ($1, $2, 'pending_review', now()) \
         ON CONFLICT (winner_id) DO UPDATE \
             SET submission = $2, status = 'pending_review', \
                 reject_reason = NULL, submitted_at = now(), \
                 updated_at = now() \
         WHERE fixed_lottery_claims.status <> 'approved'",
    )
    .bind(winner_id)
    .bind(&fields)
    .execute(&mut *tx)
    .await
    .map_err(tx_err)?
    .rows_affected();
    if n == 0 {
        tx.rollback().await.ok();
        return Err(DomainError::Validation("该奖品已审核通过".into()));
    }
    sqlx::query(
        "UPDATE fixed_lottery_winners SET status = 'claimed' WHERE id = $1",
    )
    .bind(winner_id)
    .execute(&mut *tx)
    .await
    .map_err(tx_err)?;
    tx.commit().await.map_err(tx_err)?;
    Ok(())
}

/// 中奖人提交领取资料（HTTP 用例：库操作 + 审计）。
pub(crate) async fn submit(
    state: &std::sync::Arc<AppState>,
    uid: i64,
    winner_id: i64,
    fields: serde_json::Value,
) -> DomainResult<()> {
    submit_db(&state.repo.db, uid, winner_id, fields).await?;
    state
        .repo
        .audit(Some(uid), "lottery.claim.submit", Some(winner_id))
        .await;
    Ok(())
}

/// schema 校验：fields 键值对必须覆盖 required 字段且不超 schema 集合。
fn validate_fields(
    cfg: &serde_json::Value,
    fields: &serde_json::Value,
) -> DomainResult<()> {
    let schema = cfg
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if schema.len() > 10 {
        return Err(DomainError::Validation("领取表单字段数超限".into()));
    }
    let Some(submitted) = fields.as_object() else {
        return Err(DomainError::Validation("领取资料必须是对象".into()));
    };
    if submitted.len() > 10 {
        return Err(DomainError::Validation("领取资料字段数超限".into()));
    }
    for f in &schema {
        let key = f.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let required =
            f.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
        let val = submitted.get(key).and_then(|v| v.as_str()).unwrap_or("");
        if required && val.trim().is_empty() {
            return Err(DomainError::Validation("必填字段未填写".into()));
        }
        if val.chars().count() > 500 {
            return Err(DomainError::Validation("领取资料字段内容过长".into()));
        }
    }
    Ok(())
}

/// 管理员审核。approve=true 通过；false 驳回（可重交）。
pub(crate) async fn review(
    state: &std::sync::Arc<AppState>,
    winner_id: i64,
    approve: bool,
    reason: Option<&str>,
    operator: i64,
) -> DomainResult<()> {
    let repo = &state.repo;
    let st = if approve { "approved" } else { "rejected" };
    if !approve && reason.map(str::trim).unwrap_or("").is_empty() {
        return Err(DomainError::Validation("驳回必须填写原因".into()));
    }
    let n = sqlx::query(
        "UPDATE fixed_lottery_claims \
         SET status = $2, reject_reason = $3, \
             reviewed_by = $4, reviewed_at = now(), updated_at = now() \
         WHERE winner_id = $1 AND status = 'pending_review'",
    )
    .bind(winner_id)
    .bind(st)
    .bind(reason.map(str::trim))
    .bind(Some(operator))
    .execute(&repo.db)
    .await
    .map_err(tx_err)?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该领取不在待审核状态".into()));
    }
    repo.audit_detail(
        Some(operator),
        "lottery.claim.review",
        Some(winner_id),
        reason,
        Some(json!({"approve": approve})),
    )
    .await;
    Ok(())
}

/// 超期未提交转 expired（worker 日扫）。返回翻转条数。
pub(crate) async fn expire_overdue(db: &sqlx::PgPool) -> DomainResult<u64> {
    // 自定义奖履约终态是 satisfied（不是 fulfilled），旧口径永远扫不到；
    // 待审/被驳回且超期同样失效。approved 永久豁免。
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
    .await
    .map_err(tx_err)?
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
    .await
    .map_err(tx_err)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::validate_fields;
    use serde_json::json;

    fn cfg() -> serde_json::Value {
        json!({"fields": [
            {"key": "name", "required": true},
            {"key": "note", "required": false}
        ]})
    }

    #[test]
    fn required_fields_accepted() {
        assert!(validate_fields(&cfg(), &json!({"name": "张三"})).is_ok());
    }

    #[test]
    fn missing_required_rejected() {
        assert!(validate_fields(&cfg(), &json!({"note": "x"})).is_err());
    }

    #[test]
    fn non_object_rejected() {
        assert!(validate_fields(&cfg(), &json!(["x"])).is_err());
    }

    #[test]
    fn too_many_keys_rejected() {
        let mut v = serde_json::Map::new();
        for i in 0..11 {
            v.insert(format!("k{i}"), json!("x"));
        }
        let res = validate_fields(&cfg(), &serde_json::Value::Object(v));
        assert!(res.is_err());
    }

    #[test]
    fn overly_long_value_rejected() {
        let long = "x".repeat(501);
        assert!(validate_fields(&cfg(), &json!({"name": long})).is_err());
    }
}

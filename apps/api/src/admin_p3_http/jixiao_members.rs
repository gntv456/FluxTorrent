//! 工作组成员管理写口（0482f，自 jixiao_ops.rs 拆出守行数门禁）：
//! 移除成员（撤销本期登记）/ 手动结算（替成员发本月酬劳）。
//! 口径与 jixiao_claim（本人补领）、jixiao_loop（worker 自动结算）
//! 的 CAS 互斥对齐：settled_at IS NULL AND status=0 才可动。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::checked_balance;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[derive(Deserialize)]
struct RemoveReq {
    claim_id: i64,
    /// 移除理由（审计留痕；缺省「管理员移除」）
    #[serde(default)]
    reason: Option<String>,
}

/// 移除成员：把本期登记行置 status=3（撤销）。仅限未结算行——已发薪的
/// 行是既成事实，删了会撕裂「达标月数加成」的计数口径（历史 status=1
/// 行数参与 bonus 档位），要动已发薪行走「考核收回」类人工对账。
#[post("/admin/jixiao/remove")]
async fn jixiao_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RemoveReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("管理员移除");
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT c.user_id, t.id, u.username FROM jixiao_claims c \
         JOIN jixiao_types t ON t.id = c.type_id \
         JOIN users u ON u.id = c.user_id \
         WHERE c.id = $1 AND c.settled_at IS NULL AND c.status = 0 \
         FOR UPDATE OF c",
    )
    .bind(body.claim_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, type_id, username)) = row else {
        return Err(DomainError::Validation(
            "该登记行已结算或已撤销，只能移除未结算的成员".into(),
        ));
    };
    sqlx::query(
        "UPDATE jixiao_claims SET status = 3, settled_at = now(), \
         metrics_snapshot = metrics_snapshot \
           || jsonb_build_object('removed_by', $2, 'remove_reason', $3) \
         WHERE id = $1",
    )
    .bind(body.claim_id)
    .bind(auth.id)
    .bind(reason)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "jixiao.remove",
            Some(uid),
            Some(reason),
            Some(serde_json::json!({
                "claim_id": body.claim_id, "type_id": type_id,
            })),
        )
        .await;
    tracing::info!(claim = body.claim_id, user = uid, "jixiao 成员移除");
    Ok(ok(serde_json::json!({
        "claim_id": body.claim_id, "user_id": uid, "username": username,
    })))
}

#[derive(Deserialize)]
struct SettleReq {
    claim_id: i64,
    /// 手动覆盖实发额（缺省按岗位 base_pay + 站点加成规则算）
    #[serde(default)]
    amount_override: Option<i64>,
}

/// 手动结算：替成员发本期酬劳（成员未自己领取、worker 未跑时的补发口）。
/// 与 claim/worker 三方共用 CAS（status=0 AND settled_at IS NULL），
/// 发薪走 spark_ledger 幂等键 jixiao:admin:{claim_id}，指标快照取实时值。
#[post("/admin/jixiao/settle")]
async fn jixiao_admin_settle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row: Option<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT c.user_id, t.base_pay, c.period, u.username \
         FROM jixiao_claims c \
         JOIN jixiao_types t ON t.id = c.type_id \
         JOIN users u ON u.id = c.user_id \
         WHERE c.id = $1 AND c.settled_at IS NULL AND c.status = 0 \
         FOR UPDATE OF c",
    )
    .bind(body.claim_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, base_pay, period, username)) = row else {
        return Err(DomainError::Validation(
            "该登记行已结算或已撤销，无需再发".into(),
        ));
    };
    let amount = body.amount_override.unwrap_or(base_pay).max(0);
    let metrics = crate::ops_http::jixiao_compute::compute_metrics(
        &state.repo.db,
        uid,
        &period,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let upd = sqlx::query(
        "UPDATE jixiao_claims SET status = 1, settled_at = now(), \
                amount = $2, bonus_paid = 0, metrics_at_settle = $3, \
                metrics_snapshot = metrics_snapshot \
                  || '{\"settle_by\":\"admin\"}'::jsonb \
         WHERE id = $1 AND settled_at IS NULL AND status = 0",
    )
    .bind(body.claim_id)
    .bind(amount)
    .bind(&metrics)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if upd == 0 {
        return Err(DomainError::Validation(
            "并发冲突：该行刚被本人领取或 worker 结算".into(),
        ));
    }
    if amount > 0 {
        let idem = format!("jixiao:admin:{}", body.claim_id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger \
             WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !exists {
            let balance: Option<i64> = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(uid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(balance) = balance else {
                return Err(DomainError::NotFound(uid));
            };
            let after = checked_balance(balance, amount)?;
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, \
                 idempotency_key, balance_after) VALUES \
                 (nextval('spark_ledger_id_seq'), $1, $2, 'jixiao_reward', \
                  $3, $4)",
            )
            .bind(uid)
            .bind(amount)
            .bind(&idem)
            .bind(after)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 \
                 WHERE id = $1",
            )
            .bind(uid)
            .bind(amount)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "jixiao.admin_settle",
            Some(uid),
            None,
            Some(serde_json::json!({
                "claim_id": body.claim_id, "period": period,
                "amount": amount,
            })),
        )
        .await;
    tracing::info!(
        claim = body.claim_id,
        user = uid,
        amount,
        "jixiao 手动结算"
    );
    Ok(ok(serde_json::json!({
        "claim_id": body.claim_id, "user_id": uid, "username": username,
        "amount": amount,
    })))
}

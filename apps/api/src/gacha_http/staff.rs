//! G31-C staff 侧发放：POST /admin/gacha/grant（发券）/grant-shards（发碎片）。
//! 权限走 `user.adjust`（与 donate_admin 同口径）；幂等键必填——发放在任何
//! 情况下都不允许无幂等重放（重复发放 = 印钞）。余额调整与流水同事务成对。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
struct GrantReq {
    user_id: i64,
    amount: i64,
    note: String,
    idempotency_key: String,
}

fn validate(body: &GrantReq) -> DomainResult<()> {
    if body.amount <= 0 || body.amount > 1_000_000 {
        return Err(DomainError::Validation(
            "amount 需在 1~1,000,000".into(),
        ));
    }
    let k = body.idempotency_key.trim();
    if k.len() < 8 || k.len() > 120 {
        return Err(DomainError::Validation(
            "idempotency_key 需 8~120 字符".into(),
        ));
    }
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("note 不能为空".into()));
    }
    Ok(())
}

async fn grant_tx(
    state: &Db,
    body: &GrantReq,
    table: &str,
    ledger: &str,
    kind: &str,
) -> DomainResult<i32> {
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(&format!(
        "INSERT INTO {table} (user_id, balance) VALUES ($1, 0) \
         ON CONFLICT (user_id) DO NOTHING"
    ))
    .bind(body.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
    )
    .bind(body.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(body.user_id));
    }
    let bal: i32 = sqlx::query_scalar(&format!(
        "SELECT balance FROM {table} WHERE user_id = $1 FOR UPDATE"
    ))
    .bind(body.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let seen: Option<i32> = sqlx::query_scalar(&format!(
        "SELECT balance_after FROM {ledger} WHERE idempotency_key = $1"
    ))
    .bind(body.idempotency_key.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if seen.is_some() {
        // 幂等重放：**拒绝**而非静默返回——发放动作必须可审计，静默重放会让
        // 调用方误以为又发成功（e2e 曾抓到 500+500 双发）。改为 400 明示。
        return Err(DomainError::Validation(
            "该 idempotency_key 已发放过".into(),
        ));
    }
    let after = bal + body.amount as i32;
    sqlx::query(&format!(
        "INSERT INTO {ledger} (user_id, delta, kind, ref_type, \
         idempotency_key, balance_after) \
         VALUES ($1, $2, $3, 'staff', $4, $5)"
    ))
    .bind(body.user_id)
    .bind(body.amount)
    .bind(kind)
    .bind(body.idempotency_key.trim())
    .bind(after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(&format!(
        "UPDATE {table} SET balance = $2 WHERE user_id = $1"
    ))
    .bind(body.user_id)
    .bind(after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(after)
}

#[post("/admin/gacha/grant")]
pub(crate) async fn gacha_grant_tickets(
    state: Db,
    req: HttpRequest,
    body: web::Json<GrantReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    validate(&body)?;
    let after = grant_tx(
        &state,
        &body,
        "gacha_ticket_balance",
        "gacha_ticket_ledger",
        "grant",
    )
    .await?;
    tracing::info!(
        staff = auth.id, user = body.user_id, amount = body.amount,
        note = body.note.trim(), "gacha tickets granted"
    );
    Ok(ok(serde_json::json!({
        "userId": body.user_id, "ticketBalance": after,
        "granted": body.amount,
    })))
}

#[post("/admin/gacha/grant-shards")]
pub(crate) async fn gacha_grant_shards(
    state: Db,
    req: HttpRequest,
    body: web::Json<GrantReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    validate(&body)?;
    let after = grant_tx(
        &state,
        &body,
        "gacha_shard_balance",
        "gacha_shard_ledger",
        "grant",
    )
    .await?;
    tracing::info!(
        staff = auth.id, user = body.user_id, amount = body.amount,
        note = body.note.trim(), "gacha shards granted"
    );
    Ok(ok(serde_json::json!({
        "userId": body.user_id, "shardBalance": after,
        "granted": body.amount,
    })))
}

//! 促销 intent 审批端点（0356）：approve / reject / cancel。
//! 提交与列表在 promo_intent_list.rs；共享核心（快照/effect）在
//! promo_intent.rs。

use super::promo_intent::{promo_effect, snapshot_hash};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

/// 审批通过：事务内 ① 算快照 ② 写 promotions（source='manual'，
/// created_by=审批人）③ intent 置 approved。promotions 行的 id 回填审计。
#[post("/staff/promo-intents/{id}/approve")]
pub async fn promo_intent_approve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let iid = path.into_inner();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row = sqlx::query_as::<_, IntentRow>(
        "SELECT id, scope, torrent_id, category_id, kind, starts_at, \
                ends_at, status, requested_by \
           FROM promo_intents WHERE id = $1 FOR UPDATE",
    )
    .bind(iid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or_else(|| DomainError::Validation("intent 不存在".into()))?;
    if row.status != "pending" {
        return Err(DomainError::Validation(
            "该 intent 已处理（非待审状态）".into(),
        ));
    }
    if row.ends_at <= chrono::Utc::now() {
        sqlx::query("UPDATE promo_intents SET status='expired' WHERE id=$1")
            .bind(iid)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Err(DomainError::Validation(
            "该 intent 已过结束时间，自动置 expired".into(),
        ));
    }
    // 决定时点统一取库时钟（与 promo 裁决同源，避免应用时钟漂移）
    let decided_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT now()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let snap = snapshot_hash(
        &row.kind,
        &row.starts_at,
        &row.ends_at,
        &decided_at,
        row.requested_by,
    );
    let effect = promo_effect(&mut tx, &row.kind).await?;
    let promo_id: i64 = match row.scope.as_str() {
        "torrent" => sqlx::query_scalar(
            "INSERT INTO promotions (scope, torrent_id, kind, \
                 starts_at, ends_at, source, created_by) \
                 VALUES ('torrent', $1, $2::promotion_kind_enum, $3, \
                 $4, 'manual', $5) RETURNING id",
        )
        .bind(row.torrent_id)
        .bind(&effect)
        .bind(row.starts_at)
        .bind(row.ends_at)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
        "category" => sqlx::query_scalar(
            "INSERT INTO promotions (scope, category_id, kind, \
                 starts_at, ends_at, source, created_by) \
                 VALUES ('category', $1, $2::promotion_kind_enum, $3, \
                 $4, 'manual', $5) RETURNING id",
        )
        .bind(row.category_id)
        .bind(&effect)
        .bind(row.starts_at)
        .bind(row.ends_at)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
        s => sqlx::query_scalar(
            "INSERT INTO promotions (scope, kind, starts_at, \
                 ends_at, source, created_by) \
                 VALUES ($1::promotion_scope, $2::promotion_kind_enum, \
                 $3, $4, 'manual', $5) RETURNING id",
        )
        .bind(s)
        .bind(&effect)
        .bind(row.starts_at)
        .bind(row.ends_at)
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
    };
    sqlx::query(
        "UPDATE promo_intents SET status='approved', decided_by=$2, \
         decided_at=$3, snapshot_hash=$4 WHERE id=$1",
    )
    .bind(iid)
    .bind(auth.id)
    .bind(decided_at)
    .bind(&snap)
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
            "promo_intent_approve",
            Some(iid),
            None,
            Some(serde_json::json!({ "promo_id": promo_id })),
        )
        .await;
    Ok(ok(serde_json::json!({
        "id": iid, "promo_id": promo_id, "status": "approved",
    })))
}

/// 拒绝：置 rejected + note（note 必填，请求者需要知道原因）。
#[post("/staff/promo-intents/{id}/reject")]
pub async fn promo_intent_reject(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<RejectBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let note = body.note.trim();
    if note.is_empty() {
        return Err(DomainError::Validation("拒绝需填写理由".into()));
    }
    let iid = path.into_inner();
    let n = sqlx::query(
        "UPDATE promo_intents SET status='rejected', decided_by=$2, \
         decided_at=now(), decide_note=$3 \
         WHERE id=$1 AND status='pending'",
    )
    .bind(iid)
    .bind(auth.id)
    .bind(note.chars().take(500).collect::<String>())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("intent 不存在或已处理".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_intent_reject", Some(iid))
        .await;
    Ok(ok(serde_json::json!({
        "id": iid, "status": "rejected",
    })))
}

/// 请求者撤回自己的 pending intent（管理组可撤任意 pending）。
#[post("/staff/promo-intents/{id}/cancel")]
pub async fn promo_intent_cancel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let iid = path.into_inner();
    let staff =
        crate::authz::can(&state, &auth, crate::authz::perm::FREELEECH_MANAGE)
            .await;
    let n = sqlx::query(
        "UPDATE promo_intents SET status='cancelled', decided_by=$2, \
         decided_at=now() \
         WHERE id=$1 AND status='pending' AND ($3 OR requested_by=$2)",
    )
    .bind(iid)
    .bind(auth.id)
    .bind(staff)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "intent 不存在、已处理或非本人提交".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_intent_cancel", Some(iid))
        .await;
    Ok(ok(serde_json::json!({
        "id": iid, "status": "cancelled",
    })))
}

#[derive(sqlx::FromRow)]
struct IntentRow {
    id: i64,
    scope: String,
    torrent_id: Option<i64>,
    category_id: Option<i32>,
    kind: String,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    status: String,
    requested_by: i64,
}

#[derive(Deserialize)]
struct RejectBody {
    #[serde(default)]
    note: String,
}

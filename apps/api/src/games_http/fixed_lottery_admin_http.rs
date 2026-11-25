//! 固定抽签管理口：活动 CRUD / 发布 / 取消 / 立即开奖 / 履约重试 / 领取审核。
//! 权限：USER_ADJUST（与 arcade_admin 同闸）。只做参数适配，用例在 services。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::fixed_lottery as rp;
use crate::repo::fixed_lottery_admin as rpa;
use crate::services::fixed_lottery as svc;
use crate::state::AppState;

fn forbidden_not_staff() -> DomainError {
    DomainError::Validation("无权操作".into())
}

/// 活动全量表（draft+open+drawn+cancelled）。
#[get("/admin/lottery/campaigns")]
pub(super) async fn admin_lottery_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let rows = rpa::admin_list_all(&state.repo).await?;
    Ok(ok(serde_json::json!({"rows": rows.iter().map(|r|
        serde_json::json!({"id": r.0, "code": r.1, "title": r.2,
            "status": r.3, "trigger": r.4, "ticketPrice": r.5}))
        .collect::<Vec<_>>()})))
}

#[post("/admin/lottery/campaigns/{id}/publish")]
pub(super) async fn admin_lottery_publish(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let n = rpa::cas_publish(&state.repo, path.into_inner()).await?;
    if n == 0 {
        return Err(DomainError::Validation("活动不在草稿状态".into()));
    }
    Ok(ok(serde_json::json!({"published": true})))
}

/// 取消（draft/open→cancelled），必须带原因。
#[delete("/admin/lottery/campaigns/{id}")]
pub(super) async fn admin_lottery_cancel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CancelBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let reason = body.reason.trim();
    if reason.is_empty() {
        return Err(DomainError::Validation("取消必须填写原因".into()));
    }
    let cid = path.into_inner();
    let n = rpa::cas_cancel(&state.repo, cid, reason).await?;
    if n == 0 {
        return Err(forbidden_not_staff());
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "lottery.cancelled",
            Some(cid),
            Some(reason),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({"cancelled": true})))
}

#[derive(Deserialize)]
pub(super) struct CancelBody {
    pub reason: String,
}

/// 立即开奖（手动触发；scheduled/threshold 都可提前）。
#[post("/admin/lottery/campaigns/{id}/draw")]
pub(super) async fn admin_lottery_draw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let (already, count) =
        svc::draw(&state, path.into_inner(), Some(auth.id)).await?;
    Ok(ok(
        serde_json::json!({"alreadyDrawn": already, "winners": count}),
    ))
}

/// 履约重试（failed/exhausted 行）。
#[post("/admin/lottery/fulfillments/{winner_id}/retry")]
pub(super) async fn admin_lottery_retry(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let wid = path.into_inner();
    // 重试口：把退避计时清零后走同一 fulfill 用例
    rpa::reset_retry(&state.repo, wid).await?;
    let st = svc::fulfill::fulfill_one(&state, wid).await?;
    state
        .repo
        .audit(Some(auth.id), "lottery.retry", Some(wid))
        .await;
    Ok(ok(serde_json::json!({"status": st})))
}

/// 领取审核（approve/reject）。
#[post("/admin/lottery/claims/{winner_id}/review")]
pub(super) async fn admin_lottery_claim_review(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ClaimReviewBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    svc::claim::review(
        &state,
        path.into_inner(),
        body.approve,
        body.reason.as_deref(),
        auth.id,
    )
    .await?;
    Ok(ok(serde_json::json!({"reviewed": true})))
}

#[derive(Deserialize)]
pub(super) struct ClaimReviewBody {
    pub approve: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// 待审核领取队列。
#[get("/admin/lottery/claims")]
pub(super) async fn admin_lottery_claim_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let rows = rpa::claim_queue(&state.repo).await?;
    Ok(ok(serde_json::json!({"rows": rows.iter().map(|r|
        serde_json::json!({"winnerId": r.0, "username": r.1,
            "submission": r.2, "prizeName": r.3}))
        .collect::<Vec<_>>()})))
}

/// 中奖与履约状态（详情页 tab）。
#[get("/admin/lottery/campaigns/{id}/winners")]
pub(super) async fn admin_lottery_winners(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let cid = path.into_inner();
    let rows = rpa::admin_winners_of(&state.repo, cid).await?;
    Ok(ok(serde_json::json!({"rows": rows.iter().map(|r|
        serde_json::json!({"winnerId": r.0, "userId": r.1,
            "username": r.2, "prizeName": r.3, "slotNo": r.4,
            "position": r.5, "status": r.6, "fulfillStatus": r.7,
            "attempts": r.8, "lastError": r.9, "nextRetryAt": r.10}))
        .collect::<Vec<_>>()})))
}

/// 活动详情（管理侧：草稿也可读）。
#[get("/admin/lottery/campaigns/{id}")]
pub(super) async fn admin_lottery_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let c = rp::campaign_by_id(&state.repo, path.into_inner())
        .await?
        .ok_or(DomainError::NotFound(0))?;
    let prizes = rp::prizes_of(&state.repo, c.id).await?;
    let count = rp::participant_count(&state.repo, c.id).await?;
    Ok(ok(serde_json::json!({
        "id": c.id, "code": c.participation_code, "title": c.title,
        "intro": c.intro, "status": c.status, "trigger": c.trigger_type,
        "startsAt": c.starts_at, "drawAt": c.scheduled_draw_at,
        "fallbackAt": c.fallback_draw_at, "threshold": c.threshold_count,
        "ticketPrice": c.ticket_price, "eligibility": c.eligibility,
        "claimDefaultDays": c.claim_default_days, "drawMeta": c.draw_meta,
        "participants": count,
        "prizes": prizes.iter().map(|p| serde_json::json!({
            "id": p.0, "sortOrder": p.1, "name": p.2, "quantity": p.3,
            "rewardKind": p.4, "rewardConfig": p.5,
        })).collect::<Vec<_>>(),
    })))
}

use crate::services::fixed_lottery;

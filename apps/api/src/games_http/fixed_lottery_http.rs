//! 固定抽签用户口：列表 / 详情 / 报名 / 领取提交。
//! 只做参数适配与 service 调用（零 SQL，layer guard 纪律）；读侧投影
//! （列表/详情/中奖名单）走 repo 读函数。权限：登录即可，资格闸在 service。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::fixed_lottery as rp;
use crate::repo::fixed_lottery_read as rr;
use crate::services::fixed_lottery as svc;
use crate::state::AppState;

/// 活动列表（open + 近期 drawn）。返回精简投影供双 tab 渲染。
#[get("/games/lottery")]
pub(super) async fn lottery_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = rr::list_open_drawn(&state.repo).await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let joined = rp::has_joined(&state.repo, r.0, auth.id).await?;
        out.push(serde_json::json!({
            "code": r.1, "title": r.2, "status": r.3,
            "trigger": r.4, "drawAt": r.5, "ticketPrice": r.6,
            "voided": r.7.as_ref().and_then(|m| m.get("voided"))
                .and_then(|v| v.as_bool()).unwrap_or(false),
            "joined": joined,
        }));
    }
    Ok(ok(serde_json::json!({"rows": out})))
}

/// 活动详情：奖项 + 我的状态 + 中奖名单（drawn 时）。
#[get("/games/lottery/{code}")]
pub(super) async fn lottery_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let code = path.into_inner();
    let c = rp::campaign_by_code(&state.repo, &code)
        .await?
        .ok_or(DomainError::NotFound(0))?;
    let prizes = rp::prizes_of(&state.repo, c.id).await?;
    let count = rp::participant_count(&state.repo, c.id).await?;
    let joined = rp::has_joined(&state.repo, c.id, auth.id).await?;

    let mut my_prize: Option<serde_json::Value> = None;
    let mut winners_json: Vec<serde_json::Value> = Vec::new();
    if c.status == "drawn" {
        let rows = rr::winners_of(&state.repo, c.id).await?;
        for (
            uid,
            pid,
            uname,
            pname,
            sno,
            pos,
            st,
            sub,
            claim_st,
            reject_reason,
        ) in rows
        {
            if uid == auth.id {
                let winner_id =
                    rr::winner_id_of(&state.repo, c.id, uid).await?;
                let reward_kind = rr::reward_kind_of(&state.repo, pid).await?;
                my_prize = Some(serde_json::json!({
                    "winnerId": winner_id, "prizeName": pname,
                    "slotNo": sno, "status": st,
                    "rewardKind": reward_kind,
                    "claimStatus": claim_st,
                    "rejectReason": reject_reason,
                    "submission": sub,
                }));
            }
            winners_json.push(serde_json::json!({
                "username": uname, "prizeName": pname,
                "slotNo": sno, "position": pos, "status": st,
            }));
        }
    }

    Ok(ok(serde_json::json!({
        "code": c.participation_code, "title": c.title, "intro": c.intro,
        "status": c.status, "trigger": c.trigger_type,
        "startsAt": c.starts_at, "drawAt": c.scheduled_draw_at,
        "fallbackAt": c.fallback_draw_at, "threshold": c.threshold_count,
        "ticketPrice": c.ticket_price, "participants": count,
        "joined": joined, "claimDefaultDays": c.claim_default_days,
        "drawMeta": c.draw_meta,
        "prizes": prizes.iter().map(|p| serde_json::json!({
            "id": p.0, "sortOrder": p.1, "name": p.2,
            "quantity": p.3, "rewardKind": p.4, "rewardConfig": p.5,
        })).collect::<Vec<_>>(),
        "myPrize": my_prize,
        "winners": winners_json,
    })))
}

#[derive(Deserialize)]
pub(super) struct JoinBody {
    #[serde(default)]
    pub client_uuid: Option<String>,
}

/// 报名（service 侧完成资格/票价/幂等闸）。
#[post("/games/lottery/{code}/join")]
pub(super) async fn lottery_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<JoinBody>,
) -> DomainResult<HttpResponse> {
    let _ = &body.client_uuid;
    let auth = require_auth(&req, &state).await?;
    let ip = req.peer_addr().map(|a| a.ip().to_string()).or_else(|| {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    });
    let uname = rr::username_of(&state.repo, auth.id).await?;
    svc::join(
        &state,
        auth.id,
        &uname,
        auth.class_id,
        &path.into_inner(),
        ip.as_deref(),
    )
    .await?;
    Ok(ok(serde_json::json!({"joined": true})))
}

#[derive(Deserialize)]
pub(super) struct ClaimBody {
    #[serde(default)]
    pub fields: serde_json::Value,
}

/// 中奖人提交自定义奖品领取资料。
#[post("/games/lottery/claim/{winner_id}")]
pub(super) async fn lottery_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ClaimBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    svc::claim::submit(&state, auth.id, path.into_inner(), body.fields.clone())
        .await?;
    Ok(ok(serde_json::json!({"submitted": true})))
}

use crate::services::fixed_lottery;

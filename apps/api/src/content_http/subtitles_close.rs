//! 求字幕工作流收尾（0148 C2/C4/C3）：验收结算 + free 联动 + crew 分账。
//! worker 超时扫描在 subtitles_sweep.rs。认领/弃单/交稿在 subtitles_flow.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use sqlx::PgPool;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 验收（4→1）：发起人（或 staff，或超时自动）→ 整池结算。
/// C3 分账：crew 里 accepted 的成员按 share 拆池（每人独立幂等键），
/// 未确认/份额余量归主认领人。C4：offer_free=1 时同插 promotions。
#[post("/subtitles/requests/{id}/accept")]
pub(super) async fn subtitle_request_accept(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let staff = auth.class_id >= 90;
    // (发起人, 赏金池, 认领人, crew, offer_free, free_days, torrent_id)
    type AcceptRow = (i64, i64, i64, serde_json::Value, bool, i32, Option<i64>);
    let row: Option<AcceptRow> = sqlx::query_as(
        "SELECT user_id, bounty, COALESCE(claimed_by, user_id), crew, \
         offer_free, free_days, torrent_id FROM subtitle_requests WHERE \
         id = $1 AND status = 4",
    )
    .bind(rid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        req_owner,
        bounty,
        claimer,
        crew,
        offer_free,
        free_days,
        torrent_id,
    )) = row
    else {
        return Err(DomainError::Validation("该求字幕不在待验收状态".into()));
    };
    if !staff && auth.id != req_owner {
        return Err(DomainError::Forbidden);
    }
    // C4 free 联动的 created_by 记账人：人工验收记验收人，worker 自动验收
    // 由 worker 补挂（恒发起人/管理口径）；此处统一用发起人，避免 staff 代收
    // 时把「站方出免费」记到 staff 名下（审计追责口径 = 需求发起人）。
    let n = sqlx::query(
        "UPDATE subtitle_requests SET status = 1, paid_at = now() WHERE id \
         = $1 AND status = 4",
    )
    .bind(rid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该求字幕已被处理".into()));
    }
    // 分账（C3）：份额乘法后按人发（floor，尾差给主认领人）
    let mut paid =
        settle_crew(&state.repo.db, rid, bounty, claimer, &crew).await?;
    // free 联动（C4）：结算即给种子挂限时免费（source=subtitle 可追责回收）
    if offer_free {
        if let Some(tid) = torrent_id {
            let _ = sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, \
                 ends_at, source, created_by) VALUES ('torrent', $1, 'free', \
                 now(), now() + ($2 || ' days')::interval, 'subtitle', $3)",
            )
            .bind(tid)
            .bind(free_days.max(1).to_string())
            .bind(req_owner)
            .execute(&state.repo.db)
            .await;
            paid.push(serde_json::json!({
                "free": true, "torrent_id": tid, "days": free_days,
            }));
        }
    }
    state
        .repo
        .audit(Some(auth.id), "subreq.accept", Some(rid))
        .await;
    Ok(ok(serde_json::json!({
        "id": rid, "status": 1, "settlement": paid,
    })))
}

/// crew 分账核心：[{user_id, share, accepted}]，只发 accepted=true 的；
/// 余量（含未确认者份额）归主认领人。每人幂等键 subtitle-bounty-pay:{rid}:{uid}。
pub(super) async fn settle_crew(
    db: &PgPool,
    rid: i64,
    bounty: i64,
    claimer: i64,
    crew: &serde_json::Value,
) -> DomainResult<Vec<serde_json::Value>> {
    let mut paid = Vec::new();
    if bounty <= 0 {
        return Ok(paid);
    }
    let mut remainder = bounty;
    let mut shared_out: i64 = 0;
    if let Some(list) = crew.as_array() {
        for m in list.iter().filter(|m| {
            m.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false)
        }) {
            let (Some(uid), Some(share)) = (
                m.get("user_id").and_then(|v| v.as_i64()),
                m.get("share").and_then(|v| v.as_i64()),
            ) else {
                continue;
            };
            let amount = bounty * share / 100;
            if amount <= 0 || uid == claimer {
                continue;
            }
            let idem = format!("subtitle-bounty-pay:{}:{}", rid, uid);
            earn_spark(db, uid, amount, "subtitle_bounty", &idem).await?;
            remainder -= amount;
            shared_out += share;
            paid.push(serde_json::json!({
                "user_id": uid, "amount": amount, "share": share,
            }));
        }
    }
    if remainder > 0 {
        let idem = format!("subtitle-bounty-pay:{}:{}", rid, claimer);
        earn_spark(db, claimer, remainder, "subtitle_bounty", &idem).await?;
        paid.push(serde_json::json!({
            "user_id": claimer, "amount": remainder,
            "share": 100 - shared_out,
        }));
    }
    Ok(paid)
}

/// crew 成员确认邀请（认领后随时可确认；确认与否决定结算份额归属）
#[post("/subtitles/requests/{id}/crew-accept")]
pub(super) async fn subtitle_request_crew_accept(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let crew: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT crew FROM subtitle_requests WHERE id = $1 AND status IN (3, 4)",
    )
    .bind(rid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(mut crew) = crew else {
        return Err(DomainError::Validation("该求字幕不在协作状态".into()));
    };
    let Some(list) = crew.as_array_mut() else {
        return Ok(ok(serde_json::json!({ "id": rid, "crew": crew })));
    };
    let mut hit = false;
    for m in list.iter_mut() {
        if m.get("user_id").and_then(|v| v.as_i64()) == Some(auth.id) {
            if let Some(obj) = m.as_object_mut() {
                obj.insert("accepted".into(), serde_json::json!(true));
                hit = true;
            }
        }
    }
    if !hit {
        return Err(DomainError::Validation("你没有该求字幕的协作邀请".into()));
    }
    sqlx::query("UPDATE subtitle_requests SET crew = $2 WHERE id = $1")
        .bind(rid)
        .bind(&crew)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "subreq.crew_accept", Some(rid))
        .await;
    Ok(ok(serde_json::json!({ "id": rid, "crew": crew })))
}

/// 交稿（3→4）：认领人上传过审字幕后挂单；发起人收到验收通知。
#[derive(serde::Deserialize)]
pub(super) struct SubReqDeliverBody {
    subtitle_id: i64,
}

#[post("/subtitles/requests/{id}/deliver")]
pub(super) async fn subtitle_request_deliver(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<SubReqDeliverBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let staff = auth.class_id >= 90;
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM subtitles WHERE id = $1 AND deleted_at IS \
         NULL AND status = 1",
    )
    .bind(body.subtitle_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(sub_owner) = owner else {
        return Err(DomainError::NotFound(body.subtitle_id));
    };
    if sub_owner != auth.id {
        return Err(DomainError::Validation(
            "只能交付本人上传且已过审的字幕".into(),
        ));
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 4, fulfilled_subtitle_id \
         = $2, deliver_at = now() WHERE id = $1 AND status = 3 AND ($3 OR \
         claimed_by = $4) RETURNING user_id",
    )
    .bind(rid)
    .bind(body.subtitle_id)
    .bind(staff)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((req_owner,)) = row else {
        return Err(DomainError::Validation(
            "该求字幕不在认领状态或不在你名下".into(),
        ));
    };
    let accept_days: i32 = super::subtitles_util::subtitle_setting(
        &state.repo.db,
        "subtitle_accept_timeout_days",
        "3",
    )
    .await?
    .parse()
    .unwrap_or(3);
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES (NULL, $1, '字幕已交稿，请验收', $2)",
    )
    .bind(req_owner)
    .bind(format!(
        "你发起的求字幕 #{rid} 已交稿（字幕 #{sid}）。请在 {accept_days} 天内验收；\
         逾期将自动验收结算。§§URL§§/subtitles",
        sid = body.subtitle_id
    ))
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "subreq.deliver", Some(rid))
        .await;
    Ok(ok(serde_json::json!({
        "id": rid, "status": 4, "auto_accept_days": accept_days,
    })))
}

//! 求字幕工作流收尾（0148 C2/C4/C3）：验收结算 + free 联动 + crew 分账。
//! worker 超时扫描在 subtitles_sweep.rs。认领/弃单/交稿在 subtitles_flow.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use sqlx::PgPool;

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, SpendOutcome};
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
    // P-412：状态翻转与分账必须同一事务。旧写法 CAS `4→1` 先 autocommit 落库，
    // 之后才逐个 earn_spark——任一处失败（`.await?` 上抛）这单已是 status=1 而钱
    // 没出，重试撞 `status = 4` ⇒ 「不在待验收状态」永久挡回：凑池的魔力冻在单里、
    // 译者一分没拿、界面却显示「已验收」。同型口径见 P-364 / P-411。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // (发起人, 赏金池, 认领人, crew, offer_free, free_days, torrent_id)
    type AcceptRow = (i64, i64, i64, serde_json::Value, bool, i32, Option<i64>);
    // CAS 与取行一步做完：旧实现先 SELECT 再 UPDATE，两步之间被人抢先就会拿过期
    // 行判权。判权失败/无行都在事务内返回 ⇒ 整笔自动回滚，不留半个状态。
    let row: Option<AcceptRow> = sqlx::query_as::<_, AcceptRow>(
        "UPDATE subtitle_requests SET status = 1, paid_at = now() WHERE \
         id = $1 AND status = 4 RETURNING user_id, bounty, \
         COALESCE(claimed_by, user_id), crew, offer_free, free_days, \
         torrent_id",
    )
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner, bounty, claimer, crew, offer_free, free_days, tid)) = row
    else {
        return Err(DomainError::Validation("该求字幕不在待验收状态".into()));
    };
    if !staff && auth.id != owner {
        return Err(DomainError::Forbidden);
    }
    // C4 free 联动的 created_by 记账人统一用发起人：staff 代收时不该把
    // 「站方出免费」记到 staff 名下（审计追责口径 = 需求发起人）。
    let mut paid = settle_crew_tx(&mut tx, rid, bounty, claimer, &crew).await?;
    // 免费联动进同一事务：挂了就该挂，`let _ =` 吞失败＝承诺的限时免费凭空消失
    if offer_free {
        if let Some(tid) = tid {
            sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, \
                 ends_at, source, created_by) VALUES ('torrent', $1, 'free', \
                 now(), now() + ($2 || ' days')::interval, 'subtitle', $3)",
            )
            .bind(tid)
            .bind(free_days.max(1).to_string())
            .bind(owner)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            paid.push(serde_json::json!({
                "free": true, "torrent_id": tid, "days": free_days,
            }));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "subreq.accept", Some(rid))
        .await;
    Ok(ok(serde_json::json!({
        "id": rid, "status": 1, "settlement": paid,
    })))
}

/// crew 分账核心（P-412 起在调用方事务内出账）：`[{user_id, share, accepted}]`
/// 只发 accepted=true 的；余量（含未确认者份额）归主认领人。每人独立幂等键
/// `subtitle-bounty-pay:{rid}:{uid}`（与交稿/worker 自动验收同键域，防双发）。
pub(super) async fn settle_crew_tx(
    tx: &mut sqlx::PgTransaction<'_>,
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
            pay_bounty(tx, uid, amount, rid).await?;
            remainder -= amount;
            shared_out += share;
            paid.push(serde_json::json!({
                "user_id": uid, "amount": amount, "share": share,
            }));
        }
    }
    if remainder > 0 {
        pay_bounty(tx, claimer, remainder, rid).await?;
        paid.push(serde_json::json!({
            "user_id": claimer, "amount": remainder,
            "share": 100 - shared_out,
        }));
    }
    Ok(paid)
}

/// 单笔记账：非 Spent（幂等键已在册）＝这单此前真结过，整笔回滚远比
/// 「一半人到账、一半没到」诚实。
async fn pay_bounty(
    tx: &mut sqlx::PgTransaction<'_>,
    uid: i64,
    amount: i64,
    rid: i64,
) -> DomainResult<()> {
    let idem = format!("subtitle-bounty-pay:{}:{}", rid, uid);
    let outcome =
        earn_spark_tx(tx, uid, amount, "subtitle_bounty", &idem).await?;
    if !matches!(outcome, SpendOutcome::Spent) {
        return Err(DomainError::Validation(
            "该单的赏金已发放过，未重复出账".into(),
        ));
    }
    Ok(())
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
    // 相关性判据（P-177 同族）：交稿同样必须与这一单要的是同一件事。
    // 漏了它时，「逾期自动验收结算」会把错货也一并结掉（发起人不动就发钱）。
    super::subtitles_util::ensure_req_match(
        &state.repo.db,
        rid,
        body.subtitle_id,
    )
    .await?;
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
        "INSERT INTO messages (sender_id, receiver_id, subject, body, kind) \
         VALUES (NULL, $1, '字幕已交稿，请验收', $2, 'subtitle_delivered')",
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

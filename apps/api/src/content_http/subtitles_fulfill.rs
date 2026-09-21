//! 旧直交付通道（subtitle_workflow=0 的 NexusPHP 口径，保留兼容）：
//! 0148 起结算核心复用 subtitles_close::settle_crew（crew 恒空 → 整池给交付人）。
//! 幂等键 subtitle-bounty-pay:{rid}:{uid}（与认领流程同键域，防双发）。
//! 认领/弃单/交稿在 subtitles_flow.rs；验收结算在 subtitles_close.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Deserialize)]
struct FulfillSubReq {
    subtitle_id: i64,
}

/// 旧直交付通道（subtitle_workflow=0 的 NexusPHP 口径，保留兼容）：
/// 0148 起结算核心复用 subtitles_close::settle_crew（crew 恒空 → 整池给交付人）。
/// 幂等键 subtitle-bounty-pay:{rid}:{uid}（与认领流程同键域，防双发）。
#[post("/subtitles/requests/{id}/fulfill")]
pub(super) async fn subtitle_request_fulfill(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<FulfillSubReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let sub: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM subtitles WHERE id = $1 AND deleted_at IS NULL \
         AND status = 1",
    )
    .bind(body.subtitle_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(sub_owner) = sub else {
        return Err(DomainError::NotFound(body.subtitle_id));
    };
    if sub_owner != auth.id {
        return Err(DomainError::Validation(
            "只有该字幕的上传者可以认领交付".into(),
        ));
    }
    // CAS：0 → 1（并发双交付只成功一个）。
    // (赏金池, crew, offer_free, free_days, torrent_id)
    type FulfillRow = (i64, serde_json::Value, bool, i32, Option<i64>);
    let row: Option<FulfillRow> = sqlx::query_as::<_, FulfillRow>(
        "UPDATE subtitle_requests SET status = 1, fulfilled_subtitle_id \
         = $2, paid_at = now() WHERE id = $1 AND status = 0 RETURNING \
         bounty, crew, offer_free, free_days, torrent_id",
    )
    .bind(rid)
    .bind(body.subtitle_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (bounty, crew, offer_free, free_days, torrent_id) =
        row.ok_or(DomainError::Validation("该求字幕已处理".into()))?;
    let mut paid = super::subtitles_close::settle_crew(
        &state.repo.db,
        rid,
        bounty,
        auth.id,
        &crew,
    )
    .await?;
    if offer_free {
        if let Some(tid) = torrent_id {
            let _ = sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, \
                 ends_at, source, created_by) VALUES ('torrent', $1, 'free', \
                 now(), now() + ($2 || ' days')::interval, 'subtitle', $3)",
            )
            .bind(tid)
            .bind(free_days.max(1).to_string())
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
            paid.push(serde_json::json!({
                "free": true, "torrent_id": tid, "days": free_days,
            }));
        }
    }
    Ok(ok(serde_json::json!({
        "request_id": rid, "bounty_paid": bounty, "settlement": paid,
    })))
}

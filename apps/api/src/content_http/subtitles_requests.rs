//! 求字幕悬赏 pots（0146 P2-1，KG 口径；抄 bounty.rs 的 CAS + 幂等键）：
//! 发起（初始 bounty 冻结）→ 众人 contribute 凑魔力入池 → 译者上传并认领
//! fulfill → 整池一次性结算。从 subtitles_meta.rs 按域拆出（300 行门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::trim_opt;
use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark, spend_spark_tx};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct SubtitleReqCreateReq {
    lang: String,
    #[serde(default)]
    torrent_id: Option<i64>,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    bounty: i64,
}

/// 发起求字幕：初始 bounty 即时冻结（spend_spark_tx 同事务，含建单回滚语义）
#[post("/subtitles/requests")]
pub(super) async fn subtitle_request_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubtitleReqCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let lang = body.lang.trim().to_string();
    if lang.is_empty() {
        return Err(DomainError::Validation("语言不能为空".into()));
    }
    if body.bounty < 0 {
        return Err(DomainError::Validation("悬赏不能为负".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitle_requests (user_id, torrent_id, lang, descr, \
         bounty, contributors) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.torrent_id.filter(|v| *v > 0))
    .bind(&lang)
    .bind(trim_opt(&body.descr))
    .bind(body.bounty)
    .bind(if body.bounty > 0 {
        serde_json::json!([{ "user_id": auth.id, "amount": body.bounty }])
    } else {
        serde_json::json!([])
    })
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if body.bounty > 0 {
        let idem = format!("sub-req-bounty:{}:{}", auth.id, id);
        if !matches!(
            spend_spark_tx(
                &mut tx,
                auth.id,
                body.bounty,
                "subtitle_request",
                &idem,
                "subtitle_request",
                id,
            )
            .await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "该笔请求已受理，请勿重复提交".into(),
            ));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "id": id, "bounty_frozen": body.bounty }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleReqRow {
    id: i64,
    username: Option<String>,
    torrent_id: Option<i64>,
    lang: String,
    descr: Option<String>,
    bounty: i64,
    contributors: serde_json::Value,
    status: i16,
    fulfilled_subtitle_id: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 求字幕列表（status=open 进行中默认；all 全部）
#[get("/subtitles/requests")]
pub(super) async fn subtitle_request_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl actix_web::Responder> {
    let status = q.get("status").map(String::as_str).unwrap_or("open");
    let cond = match status {
        "all" => "TRUE",
        _ => "r.status = 0",
    };
    let rows: Vec<SubtitleReqRow> = sqlx::query_as(
        &format!(
            "SELECT r.id, u.username, r.torrent_id, r.lang, r.descr, \
             r.bounty, r.contributors, r.status, r.fulfilled_subtitle_id, \
             r.created_at FROM subtitle_requests r LEFT JOIN users u ON \
             u.id = r.user_id WHERE {cond} ORDER BY r.id DESC LIMIT 100"
        ),
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ContributeReq {
    #[serde(default)]
    amount: i64,
}

/// 凑魔力入池（一人可追投；扣款走 spend_spark 幂等键锚定请求+用户）
#[post("/subtitles/requests/{id}/contribute")]
pub(super) async fn subtitle_request_contribute(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ContributeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    if body.amount <= 0 {
        return Err(DomainError::Validation("入池金额需为正整数".into()));
    }
    let bounty: i64 = sqlx::query_scalar(
        "UPDATE subtitle_requests SET bounty = bounty + $2, contributors = \
         contributors::jsonb || $3::jsonb WHERE id = $1 AND status = 0 \
         RETURNING bounty",
    )
    .bind(rid)
    .bind(body.amount)
    .bind(serde_json::json!([{ "user_id": auth.id, "amount": body.amount }]))
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::NotFound(rid))?;
    let idem = format!("sub-req-contrib:{}:{}", auth.id, rid);
    if matches!(
        spend_spark(
            &state.repo.db,
            auth.id,
            body.amount,
            "subtitle_request",
            &idem,
            "subtitle_request",
            rid,
        )
        .await?,
        crate::economy_http::SpendOutcome::Replayed
    ) {
        return Err(DomainError::Validation(
            "该笔入池已受理，请勿重复提交".into(),
        ));
    }
    Ok(ok(serde_json::json!({ "id": rid, "bounty": bounty })))
}

#[derive(Deserialize)]
struct FulfillSubReq {
    subtitle_id: i64,
}

/// 译者交付：字幕须为本人上传 → status 0→1（CAS）→ 整池一次性结算给译者。
/// 幂等键 `subtitle-bounty-pay:{req_id}`（重复结算只发一次，A11）。
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
    // CAS：0 → 1（并发双交付只成功一个）
    let paid: i64 = sqlx::query_scalar(
        "UPDATE subtitle_requests SET status = 1, fulfilled_subtitle_id = \
         $2, paid_at = now() WHERE id = $1 AND status = 0 RETURNING bounty",
    )
    .bind(rid)
    .bind(body.subtitle_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Validation("该求字幕已处理".into()))?;
    if paid > 0 {
        let idem = format!("subtitle-bounty-pay:{rid}");
        earn_spark(
            &state.repo.db,
            auth.id,
            paid,
            "subtitle_bounty",
            &idem,
        )
        .await?;
    }
    Ok(ok(serde_json::json!({
        "request_id": rid, "bounty_paid": paid,
    })))
}

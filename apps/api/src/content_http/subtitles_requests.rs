//! 求字幕悬赏 pots（0146 P2-1，KG 口径；抄 bounty.rs 的 CAS + 幂等键）：
//! 发起（初始 bounty 冻结）→ 众人 contribute 凑魔力入池 → 译者上传并认领
//! fulfill → 整池一次性结算。从 subtitles_meta.rs 按域拆出（300 行门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::trim_opt;
use crate::dto::ok;
use crate::economy_http::{spend_spark, spend_spark_tx};
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
    /// C4：交付时给种子挂限时免费（站方出，source=subtitle）
    #[serde(default)]
    offer_free: bool,
    #[serde(default = "default_free_days")]
    free_days: i32,
}

fn default_free_days() -> i32 {
    30
}

/// 认领请求体（C2/C3；crew 可选）
#[derive(Deserialize)]
pub(super) struct SubReqClaimBody {
    #[serde(default)]
    pub(super) crew: Option<Vec<SubReqCrew>>,
}

#[derive(Deserialize)]
pub(super) struct SubReqCrew {
    pub(super) user_id: i64,
    /// 1-100，合计 ≤100；余量归主认领人
    pub(super) share: i64,
    /// 翻译/校对/时间轴/后期（自由文本，≤20 字）
    pub(super) role: String,
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
         bounty, contributors, offer_free, free_days) VALUES ($1, $2, $3, \
         $4, $5, $6, $7, $8) RETURNING id",
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
    .bind(body.offer_free)
    .bind(body.free_days.clamp(1, 90))
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
    /// 0148 工作流字段
    claimed_by: Option<i64>,
    deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    deliver_at: Option<chrono::DateTime<chrono::Utc>>,
    offer_free: bool,
    free_days: i32,
    crew: serde_json::Value,
}

/// 求字幕列表（status=open 进行中默认；all 全部；claimed 我认领的）
#[get("/subtitles/requests")]
pub(super) async fn subtitle_request_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl actix_web::Responder> {
    let me = require_auth(&req, &state).await.ok();
    let status = q.get("status").map(String::as_str).unwrap_or("open");
    let cond = match status {
        "all" => "TRUE".to_string(),
        "mine" => match me {
            Some(ref a) => format!("r.user_id = {}", a.id),
            None => "FALSE".to_string(),
        },
        "claimed" => match me {
            Some(ref a) => {
                format!("r.claimed_by = {} OR r.status = 4", a.id)
            }
            None => "FALSE".to_string(),
        },
        _ => "r.status = 0".to_string(),
    };
    let rows: Vec<SubtitleReqRow> = sqlx::query_as(
        &format!(
            "SELECT r.id, u.username, r.torrent_id, r.lang, r.descr, \
             r.bounty, r.contributors, r.status, r.fulfilled_subtitle_id, \
             r.created_at, r.claimed_by, r.deadline_at, r.deliver_at, \
             r.offer_free, r.free_days, r.crew FROM subtitle_requests r \
             LEFT JOIN users u ON u.id = r.user_id WHERE {cond} ORDER BY \
             r.id DESC LIMIT 100"
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


//! 求字幕悬赏 pots（0146 P2-1，KG 口径；抄 bounty.rs 的 CAS + 幂等键）：
//! 发起（初始 bounty 冻结）→ 众人 contribute 凑魔力入池 → 译者上传并认领
//! fulfill → 整池一次性结算。从 subtitles_meta.rs 按域拆出（300 行门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::{check_request_inputs, trim_opt};
use crate::dto::ok;
use crate::economy_http::spend_spark_tx;
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
    check_request_inputs(&state.repo.db, &lang, body.descr.as_deref()).await?;
    if body.bounty < 0 {
        return Err(DomainError::Validation("悬赏不能为负".into()));
    }
    // 种子必须真实存在：`filter(|v| *v > 0)` 之后直接进 INSERT ⇒ 撞外键就是
    // 500（与批四十三 P-171「改档不查字典」同族：值域是表里的集合时，判据
    // 必须查表，不能靠外键替业务报错）。0/负数仍按「不挂种子」处理。
    if let Some(tid) = body.torrent_id.filter(|v| *v > 0) {
        // 不只是存在性：与详情同一可见性口径（待审/被拒只放行本人与
        // staff，软删谁都不可见）。旧口径下悬赏会冻在一颗公开列表里
        // 看不到的种上，兑现的字幕也进不了公开列表。
        let vis = crate::torrents::visibility_sql(Some((
            auth.id,
            auth.class_id >= 90,
        )));
        let exists: bool = sqlx::query_scalar(&format!(
            "SELECT EXISTS(SELECT 1 FROM torrents t \
             WHERE t.id = $1 AND {vis})",
        ))
        .bind(tid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !exists {
            return Err(DomainError::Validation(format!(
                "种子 #{tid} 不存在或不可见（求字幕可留空不挂种子）"
            )));
        }
    }
    // B2：offer_free 是站方出资给种子挂免费（promotions 本是 staff 专属权限），
    // 不得让普通用户借求字幕动线下放——发起时非 staff 一律拒绝。
    if body.offer_free && auth.class_id < 90 {
        return Err(DomainError::Validation(
            "限时免费由管理组设置，发起求字幕不能勾选".into(),
        ));
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
    q: crate::http::Q<std::collections::HashMap<String, String>>,
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
    let rows: Vec<SubtitleReqRow> = sqlx::query_as(&format!(
        "SELECT r.id, u.username, r.torrent_id, r.lang, r.descr, \
             r.bounty, r.contributors, r.status, r.fulfilled_subtitle_id, \
             r.created_at, r.claimed_by, r.deadline_at, r.deliver_at, \
             r.offer_free, r.free_days, r.crew FROM subtitle_requests r \
             LEFT JOIN users u ON u.id = r.user_id WHERE {cond} ORDER BY \
             r.id DESC LIMIT 100"
    ))
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

/// 凑魔力入池（一人可追投；扣款走 spend_spark_tx 幂等键锚定请求+用户+笔次）。
/// 审计修复（并发双计）：原实现先 UPDATE bounty 再扣款，同用户并发两笔时
/// 流水幂等只挡住一笔扣款、bounty 却加两次（池被灌水）。
/// P-414（本批）：「先扣款、成功才入池」仍是两个 autocommit——扣款成功了而
/// UPDATE 拿不到行（单刚被关/撤销）时，收尾靠一条 `let _ =` 的补偿退款，退款
/// 一失败＝「钱扣了、池没进、退款也没落账」的静默资损（同族 P-323 / P-406）。
/// 现在扣款、行锁、入池在同一事务：拿不到行就整笔回滚，压根不需要补偿退款。
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
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 笔次 = 该用户此前对此单的入池次数（contributors 数组里本人条数）。
    // FOR UPDATE 把这把锁加在本单行上：并发两笔不会再算出同一个 seq。
    let cur: serde_json::Value = sqlx::query_scalar(
        "SELECT contributors FROM subtitle_requests WHERE id = $1 AND \
         status = 0 FOR UPDATE",
    )
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::NotFound(rid))?;
    let seq = cur
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|m| {
                    m.get("user_id").and_then(|v| v.as_i64()) == Some(auth.id)
                })
                .count()
        })
        .unwrap_or(0);
    // 扣款（幂等键带笔次：同用户第 N 笔唯一；并发重放同笔被挡）
    let idem = format!("sub-req-contrib:{}:{}:{}", auth.id, rid, seq);
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "subtitle_request",
            &idem,
            "subtitle_request",
            rid,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔入池已受理，请勿重复提交".into(),
        ));
    }
    // 扣款与入池同事务：无行（单被关）⇒ 上面的扣款一并回滚
    let bounty: Option<i64> = sqlx::query_scalar(
        "UPDATE subtitle_requests SET bounty = bounty + $2, contributors = \
         contributors::jsonb || $3::jsonb WHERE id = $1 AND status = 0 \
         RETURNING bounty",
    )
    .bind(rid)
    .bind(body.amount)
    .bind(serde_json::json!([{ "user_id": auth.id, "amount": body.amount }]))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(bounty) = bounty else {
        return Err(DomainError::NotFound(rid));
    };
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": rid, "bounty": bounty })))
}

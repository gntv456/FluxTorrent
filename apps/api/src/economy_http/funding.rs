//! 定向众筹免费（0078 HDBits Featured 口径）。
//! 从 economy_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 定向众筹免费（0078，HDBits Featured 口径，v3 §27-13） ============
// 某个种子社区凑火花 → 达标自动挂 free×hours；到期未达标全额退款（含税部分一并退）。
// 赠送税（gift_tax_bp）对众筹同样适用：抽税入站免池，与礼物共用回收通道（v3 §27-22）。

#[derive(sqlx::FromRow, serde::Serialize)]
struct FundingRow {
    id: i64,
    torrent_id: i64,
    torrent_name: Option<String>,
    goal: i64,
    raised: i64,
    backers: i64,
    hours: i32,
    status: i16,
    ends_at: chrono::DateTime<chrono::Utc>,
}

/// 进行中/已完成的众筹列表（新→旧；种子名带出）
#[get("/fundings")]
async fn fundings_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let status: Option<i16> = q
        .get("status")
        .and_then(|s| s.parse::<i16>().ok())
        .filter(|s| (0..=3).contains(s));
    let rows = sqlx::query_as::<_, FundingRow>(
        "SELECT f.id, f.torrent_id, t.name AS torrent_name, f.goal, f.raised, \
                (SELECT count(*)::bigint FROM funding_contribs c WHERE c.funding_id = f.id) AS backers, \
                f.hours, f.status, f.ends_at \
         FROM fundings f LEFT JOIN torrents t ON t.id = f.torrent_id \
         WHERE ($1::smallint IS NULL OR f.status = $1) \
         ORDER BY f.status ASC, f.ends_at DESC LIMIT 50",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FundingCreateReq {
    torrent_id: i64,
    goal: i64,
    #[serde(default = "default_funding_hours")]
    hours: i32,
    #[serde(default = "default_funding_days")]
    days: i32,
}

fn default_funding_hours() -> i32 {
    168
}
fn default_funding_days() -> i32 {
    14
}

/// 发起众筹（种子发布者或 staff；同种子同时只能有一个进行中的众筹）
#[post("/fundings")]
async fn funding_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FundingCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.goal < 1000 {
        return Err(DomainError::Validation("众筹目标至少 1000 魔力".into()));
    }
    if !(1..=720).contains(&body.hours) || !(1..=60).contains(&body.days) {
        return Err(DomainError::Validation(
            "hours 需在 1-720、days 需在 1-60 之间".into(),
        ));
    }
    let torrent: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, _)) = torrent else {
        return Err(DomainError::NotFound(body.torrent_id));
    };
    if auth.id != owner_id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fundings WHERE torrent_id = $1 \
         AND status = 0)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open {
        return Err(DomainError::Validation("该种子已有进行中的众筹".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO fundings (torrent_id, creator_id, goal, hours, ends_at) \
         VALUES ($1, $2, $3, $4, now() + make_interval(days => $5)) RETURNING id",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(body.goal)
    .bind(body.hours)
    .bind(body.days)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "funding_create", Some(id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": id, "goal": body.goal, "hours": body.hours }),
    ))
}

// ============ 后台管理（闭环审查 C11：此前无订单/取消入口，站长只能改库） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminFundingRow {
    id: i64,
    torrent_id: i64,
    torrent_name: Option<String>,
    creator_id: i64,
    creator_name: Option<String>,
    goal: i64,
    raised: i64,
    backers: i64,
    hours: i32,
    status: i16,
    ends_at: chrono::DateTime<chrono::Utc>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /admin/fundings：全量众筹（可按状态筛）——含发起人、进度、参与人数。
#[get("/admin/fundings")]
async fn admin_fundings_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    let status: Option<i16> = q
        .get("status")
        .and_then(|s| s.parse::<i16>().ok())
        .filter(|s| (0..=3).contains(s));
    let rows = sqlx::query_as::<_, AdminFundingRow>(
        "SELECT f.id, f.torrent_id, t.name AS torrent_name, f.creator_id, \
                u.username AS creator_name, f.goal, f.raised, \
                (SELECT count(*)::bigint FROM funding_contribs c \
                  WHERE c.funding_id = f.id) AS backers, \
                f.hours, f.status, f.ends_at, f.created_at \
         FROM fundings f \
         LEFT JOIN torrents t ON t.id = f.torrent_id \
         LEFT JOIN users u ON u.id = f.creator_id \
         WHERE ($1::smallint IS NULL OR f.status = $1) \
         ORDER BY f.status ASC, f.created_at DESC LIMIT 200",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FundingCancelReq {
    id: i64,
    /// 是否给参与者退款（默认 true）。false 用于「已达标后关闭」等不退场景。
    #[serde(default = "default_true_cancel")]
    refund: bool,
}

fn default_true_cancel() -> bool {
    true
}

/// POST /admin/fundings/cancel：管理员关闭进行中的众筹。
/// 退款语义与 worker settle 完全一致（逐笔幂等退 spark_ledger + 余额），
/// 最终置 status=2（对参与者等价于「未达标退款」）；幂等键前缀 funding-refund
/// 与 worker 共用，重复触发不会双退。通知发起人 + 参与者。
#[post("/admin/fundings/cancel")]
async fn admin_funding_cancel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FundingCancelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    let db = &state.repo.db;
    // 仅进行中（status=0）可被管理员关闭；已达标/已结束不允许（防误退已挂促销的）
    let cur: Option<i16> =
        sqlx::query_scalar("SELECT status FROM fundings WHERE id = $1")
            .bind(body.id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(status) = cur else {
        return Err(DomainError::NotFound(body.id));
    };
    if status != 0 {
        return Err(DomainError::Validation(
            "仅进行中的众筹可关闭".into(),
        ));
    }
    let mut refunded = 0u64;
    if body.refund {
        let contribs: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT user_id, amount FROM funding_contribs \
             WHERE funding_id = $1",
        )
        .bind(body.id)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        for (uid, amount) in &contribs {
            let idem = format!("funding-refund:{}:{uid}", body.id);
            sqlx::query(
                r#"INSERT INTO spark_ledger
                   (id, user_id, amount, kind, idempotency_key)
                   SELECT nextval('spark_ledger_id_seq'), $1, $2,
                          'funding_refund', $3
                   WHERE NOT EXISTS (SELECT 1 FROM spark_ledger
                     WHERE idempotency_key = $3)"#,
            )
            .bind(uid)
            .bind(amount)
            .bind(&idem)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 \
                 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(amount)
            .bind(&idem)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            refunded += 1;
        }
    }
    sqlx::query("UPDATE fundings SET status = 2 WHERE id = $1")
        .bind(body.id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let note = if body.refund {
        "'众筹已被管理员关闭，参与者的魔力已全额退款。'"
    } else {
        "'众筹已被管理员关闭。'"
    };
    let _ = sqlx::query(&format!(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT NULL, creator_id, '众筹已关闭', {note} \
         FROM fundings WHERE id = $1"
    ))
    .bind(body.id)
    .execute(db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "funding_cancel", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({
        "id": body.id, "refunded": refunded, "refund": body.refund,
    })))
}

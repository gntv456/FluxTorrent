//! 促销 intent 端点（0356）：提交 / 列表。
//! 审批（approve/reject/cancel）在 promo_intent_approve.rs；
//! 快照与直挂留痕的共享核心在 promo_intent.rs。

use super::promo::promo_parse;
use super::promo_intent::{promo_effect, PromoIntentBody};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, Responder};

/// 提交促销 intent：发布者为自己种子申请（scope 恒 'torrent'），
/// 或管理组为任意范围预登记（免审批场景走 freeleech_set 直挂留痕，
/// 此端点提交的站点级 intent 仍需审批——管理组自己也想留决策痕时用）。
#[post("/staff/promo-intents")]
pub async fn promo_intent_submit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoIntentBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 权限：发布者可为「自己的种子」申请；管理组（FREELEECH_MANAGE）
    // 可为任意范围提交（审批同样需要该权限，见 approve）。
    let staff =
        crate::authz::can(&state, &auth, crate::authz::perm::FREELEECH_MANAGE)
            .await;
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    promo_effect(&mut tx, &kind).await?;
    // torrent 范围：非管理组必须是自己发布的已过审种子
    // （自购链路 0173 已放开全员，这里收紧到 owner——intent 是
    //  「作者为种子申请促销」语义，与 promo.rs 的购买消费不同）
    let mut torrent_id: Option<i64> = None;
    let scope = if scope == "torrent" || body.torrent_id.is_some() {
        let tid = body.torrent_id.ok_or_else(|| {
            DomainError::Validation("种子促销需指定 torrent_id".into())
        })?;
        let owner: Option<i64> = sqlx::query_scalar(
            "SELECT owner_id FROM torrents \
             WHERE id = $1 AND approval_status = 1",
        )
        .bind(tid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(owner) = owner else {
            return Err(DomainError::NotFound(tid));
        };
        if !staff && owner != auth.id {
            return Err(DomainError::Forbidden);
        }
        torrent_id = Some(tid);
        "torrent".to_string()
    } else {
        // 站点级范围（global/official/non_official/category）只有管理组可提
        if !staff {
            return Err(DomainError::Forbidden);
        }
        scope
    };
    if scope == "category" && body.category_id.is_none() {
        return Err(DomainError::Validation("分类促销需指定分类".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO promo_intents \
         (scope, torrent_id, category_id, kind, starts_at, ends_at, \
          status, requested_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7) RETURNING id",
    )
    .bind(&scope)
    .bind(torrent_id)
    .bind(body.category_id)
    .bind(&kind)
    .bind(starts_at)
    .bind(ends_at)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "promo_intent_submit", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id, "status": "pending" })))
}

/// 待审/历史 intent 列表（pending 优先；?status= 过滤）。
#[get("/staff/promo-intents")]
pub async fn promo_intent_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<PromoIntentQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_VIEW,
    )
    .await?;
    let status = q.status.as_deref().unwrap_or("");
    let valid = [
        "",
        "pending",
        "approved",
        "rejected",
        "cancelled",
        "expired",
    ];
    if !valid.contains(&status) {
        return Err(DomainError::Validation("状态过滤值无效".into()));
    }
    // pending 优先排序：CASE 让 pending 恒排最前，其余按新→旧
    let rows: Vec<serde_json::Value> = sqlx::query(
        "SELECT i.id, i.scope, i.torrent_id, t.name AS torrent_name, \
                i.category_id, c.name AS category_name, i.kind, \
                i.starts_at, i.ends_at, i.status, i.requested_by, \
                ru.username AS requested_by_name, i.decided_by, \
                i.decided_at, i.decide_note, i.snapshot_hash, i.created_at \
           FROM promo_intents i \
           LEFT JOIN torrents t ON t.id = i.torrent_id \
           LEFT JOIN categories c ON c.id = i.category_id \
           LEFT JOIN users ru ON ru.id = i.requested_by \
          WHERE ($1 = '' OR i.status = $1) \
          ORDER BY (i.status = 'pending') DESC, i.id DESC LIMIT 200",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|r| row_to_json(&r))
    .collect();
    Ok(ok(rows))
}

/// sqlx Row → JSON（列表列少，手写映射避免 FromRow 结构体膨胀）。
fn row_to_json(r: &sqlx::postgres::PgRow) -> serde_json::Value {
    use sqlx::Row;
    serde_json::json!({
        "id": r.try_get::<i64, _>("id").ok(),
        "scope": r.try_get::<String, _>("scope").ok(),
        "torrent_id": r.try_get::<Option<i64>, _>("torrent_id")
            .ok().flatten(),
        "torrent_name": r.try_get::<Option<String>, _>("torrent_name")
            .ok().flatten(),
        "category_id": r.try_get::<Option<i32>, _>("category_id")
            .ok().flatten(),
        "category_name": r.try_get::<Option<String>, _>("category_name")
            .ok().flatten(),
        "kind": r.try_get::<String, _>("kind").ok(),
        "starts_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>(
            "starts_at",
        ).ok(),
        "ends_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>(
            "ends_at",
        ).ok(),
        "status": r.try_get::<String, _>("status").ok(),
        "requested_by": r.try_get::<i64, _>("requested_by").ok(),
        "requested_by_name": r.try_get::<Option<String>, _>(
            "requested_by_name",
        ).ok().flatten(),
        "decided_by": r.try_get::<Option<i64>, _>("decided_by")
            .ok().flatten(),
        "decided_at": r.try_get::<
            Option<chrono::DateTime<chrono::Utc>>,
            _,
        >("decided_at").ok().flatten(),
        "decide_note": r.try_get::<Option<String>, _>("decide_note")
            .ok().flatten(),
        "snapshot_hash": r.try_get::<Option<String>, _>("snapshot_hash")
            .ok().flatten(),
        "created_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>(
            "created_at",
        ).ok(),
    })
}

#[derive(serde::Deserialize)]
struct PromoIntentQuery {
    #[serde(default)]
    status: Option<String>,
}

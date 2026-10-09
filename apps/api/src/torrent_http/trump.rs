//! 种子举报/替换（0336 trumping，PTP/GGn 口径）。
//!
//! 三件事：用户举报劣质/死种（可指名替代）→ 版主裁决 → 通过则淘汰被举报种。
//! 「本站型是否启用」由 `site_type_packs.features->>'trumping'` 决定（0336），
//! **不 match site_type** —— 自定义站型声明了能力就该生效。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

const REASONS: [&str; 5] =
    ["bad_quality", "dead", "wrong_content", "duplicate", "other"];

/// 当前站型是否启用某项能力（0336 features 的唯一消费口径）。
pub(crate) async fn feature_on(state: &AppState, key: &str) -> bool {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT p.features->>$1 FROM site_settings s \
         JOIN site_type_packs p ON p.code = s.value \
         WHERE s.name = 'site_type'",
    )
    .bind(key)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    v.as_deref() == Some("true")
}

#[derive(Deserialize)]
pub(crate) struct TrumpReq {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    target_torrent_id: Option<i64>,
}

/// 举报一个种子（可指名更优替代）。
#[post("/torrents/{id}/trump")]
pub async fn torrent_trump(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TrumpReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !feature_on(&state, "trumping").await {
        return Err(DomainError::Validation(
            "本站型未启用举报替换（trumping）能力".into(),
        ));
    }
    let id = path.into_inner();
    let reason = body.reason.as_deref().unwrap_or("other");
    if !REASONS.contains(&reason) {
        return Err(DomainError::Validation("举报理由不合法".into()));
    }
    let note = body.note.as_deref().unwrap_or("").trim();
    if note.chars().count() > 500 {
        return Err(DomainError::Validation("说明需 ≤500 字".into()));
    }
    let owner: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner, _)) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner == auth.id {
        return Err(DomainError::Validation("不能举报自己发布的种子".into()));
    }
    if let Some(t) = body.target_torrent_id {
        if t == id {
            return Err(DomainError::Validation("替代种不能是被举报种自身".into()));
        }
        let ok_target: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM torrents \
             WHERE id = $1 AND approval_status = 1)",
        )
        .bind(t)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !ok_target {
            return Err(DomainError::Validation("替代种不存在或未过审".into()));
        }
    }
    // 同一人重复举报同一 pending 单：幂等返回已存在的
    let dup: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM torrent_trumps \
         WHERE torrent_id = $1 AND reporter_id = $2 AND status = 'pending'",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(existing) = dup {
        return Ok(ok(serde_json::json!({ "id": existing, "dup": true })));
    }
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO torrent_trumps \
           (torrent_id, target_torrent_id, reporter_id, reason, note) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(id)
    .bind(body.target_torrent_id)
    .bind(auth.id)
    .bind(reason)
    .bind(note)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": tid })))
}

/// 某颗种子的举报汇总（详情页徽标数据源）。
/// 普通用户只见计数，版主（class_id ≥ 90）另见明细。
#[get("/torrents/{id}/trumps")]
pub async fn torrent_trumps(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrent_trumps \
         WHERE torrent_id = $1 AND status = 'pending'",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if auth.class_id < 90 {
        return Ok(ok(serde_json::json!({ "pending": pending })));
    }
    let rows: Vec<(i64, String, String, String, Option<i64>, String)> =
        sqlx::query_as(
            "SELECT t.id, t.reason, t.note, t.status, t.target_torrent_id, \
                    t.created_at::text \
             FROM torrent_trumps t WHERE t.torrent_id = $1 \
             ORDER BY t.created_at DESC LIMIT 50",
        )
        .bind(id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(tid, reason, note, status, target, at)| {
            serde_json::json!({
                "id": tid, "reason": reason, "note": note,
                "status": status, "target_torrent_id": target,
                "created_at": at,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "pending": pending, "items": items })))
}

/// 版主：待处理举报列表。
#[get("/admin/trumps")]
pub async fn trump_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let rows: Vec<(i64, i64, String, String, String, Option<i64>, String)> =
        sqlx::query_as(
            "SELECT t.id, t.torrent_id, t.reason, t.note, t.status, \
                    t.target_torrent_id, t.created_at::text \
             FROM torrent_trumps t WHERE t.status = 'pending' \
             ORDER BY t.created_at ASC LIMIT 100",
        )
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, tor, reason, note, status, target, at)| {
            serde_json::json!({
                "id": id, "torrent_id": tor, "reason": reason,
                "note": note, "status": status,
                "target_torrent_id": target, "created_at": at,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

#[derive(Deserialize)]
pub(crate) struct ResolveReq {
    accept: bool,
    #[serde(default)]
    note: Option<String>,
}

/// 版主裁决：accept=true 淘汰被举报种（approval_status=2 + deny_note）。
#[post("/admin/trumps/{id}/resolve")]
pub async fn trump_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let tid = path.into_inner();
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT torrent_id, status FROM torrent_trumps WHERE id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((torrent_id, status)) = row else {
        return Err(DomainError::NotFound(tid));
    };
    if status != "pending" {
        return Err(DomainError::Validation("该举报已裁决".into()));
    }
    let note = body.note.as_deref().unwrap_or("").trim().to_string();
    let new_status = if body.accept { "accepted" } else { "rejected" };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE torrent_trumps SET status = $1, handled_by = $2, \
                handled_at = now() WHERE id = $3",
    )
    .bind(new_status)
    .bind(auth.id)
    .bind(tid)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if body.accept {
        let deny = if note.is_empty() {
            "trump 裁决：本种被判定为劣质/重复，已由更优版本替代".to_string()
        } else {
            format!("trump 裁决：{note}")
        };
        sqlx::query(
            "UPDATE torrents SET approval_status = 2, deny_note = $1 \
             WHERE id = $2",
        )
        .bind(&deny)
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": tid, "status": new_status, "torrent_id": torrent_id,
    })))
}

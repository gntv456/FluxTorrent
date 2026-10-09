//! 举报裁决的管理侧面（0336 trumping，从 trump.rs 拆出守行数门禁）。
//! 版主待办队列与裁决端点；站型能力开关 `feature_on` 在 trump.rs。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
        // 同种其余 pending 单一并收口（superseded）：否则队列里会挂着
        // 指向已淘汰种的僵尸单
        sqlx::query(
            "UPDATE torrent_trumps SET status = 'superseded', \
                    handled_by = $1, handled_at = now() \
             WHERE torrent_id = $2 AND status = 'pending' AND id <> $3",
        )
        .bind(auth.id)
        .bind(torrent_id)
        .bind(tid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 公信动作进审计日志（本仓 admin 写操作的留痕口径）
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "trump.resolve",
            Some(tid),
            Some(&note),
            Some(serde_json::json!({
                "accept": body.accept, "torrent_id": torrent_id,
            })),
        )
        .await;
    // 被淘汰种可能出现在首屏缓存里，失效列表代际
    if body.accept {
        super::list::bump_list_cache_gen(&state).await;
    }
    Ok(ok(serde_json::json!({
        "id": tid, "status": new_status, "torrent_id": torrent_id,
    })))
}

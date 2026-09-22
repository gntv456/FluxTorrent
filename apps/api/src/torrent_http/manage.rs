//! 种子管理（M02/M03）：编辑/定价/恢复/重提/删除。
//! 从 torrent_http.rs 按域拆出。

use actix_web::{delete, post, put, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::interact::TorrentEditReq;

#[put("/torrents/{id}")]
async fn edit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TorrentEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    // 0150：IMDB 直填暂存（TorrentEdit.imdb_id 是 &str，借本地变量）
    let imdb_norm: Option<String> = body
        .imdb_id
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            // 空串 = 清空；非空统一大写后校验 TT+7~8 位；非法回落 None
            let up = s.to_ascii_uppercase();
            let valid = up.len() >= 9
                && up.starts_with("TT")
                && up[2..].chars().all(|c| c.is_ascii_digit());
            if s.is_empty() || valid {
                Some(up)
            } else {
                None
            }
        });
    torrents::edit_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        &torrents::TorrentEdit {
            name: body
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
            small_descr: body.small_descr.as_deref(),
            descr: body.descr.as_deref(),
            anonymous: body.anonymous,
            category_id: body.category_id,
            medium_id: body.medium_id,
            grade_id: body.grade_id,
            edition_id: body.edition_id,
            imdb_id: imdb_norm.as_deref(),
        },
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.edit", Some(tid))
        .await;
    // 0148 C1：编辑 descr 后重提取 IMDB（descr 提取得到才覆盖，否则保留旧值）
    if let Some(d) = body.descr.as_deref() {
        if let Some(imdb) = crate::publish_http::extract_imdb_pub(d) {
            let _ = sqlx::query(
                "UPDATE torrents SET imdb_id = $2 WHERE id = $1 AND \
                 (imdb_id IS NULL OR imdb_id <> $2)",
            )
            .bind(tid)
            .bind(&imdb)
            .execute(&state.repo.db)
            .await;
        }
    }
    Ok(ok(
        serde_json::json!({ "edited": true, "note": "已回退待审核" }),
    ))
}

/// 修改付费定价（0086）：发布者或 staff；改价不影响已购（以 torrent_purchases 已扣为准）
#[derive(serde::Deserialize)]
struct PriceReq {
    price: i64,
}

#[put("/torrents/{id}/price")]
async fn set_torrent_price(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<PriceReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let price = body.price.clamp(0, 1_000_000);
    sqlx::query("UPDATE torrents SET price = $2 WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "torrent.price_set", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "price": price })))
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数
#[post("/torrents/{id}/restore")]
async fn restore_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    torrents::restore_torrent(&state.repo.db, id).await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.restore", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "restored": id })))
}

/// 被拒种子修改后重提（U4 §12.5 审核闭环：rejected → pending，保留原 id 与评论区）。
/// 仅作者本人；清拒绝标记（deny_reason/note），重进审核队列。
#[post("/torrents/{id}/resubmit")]
async fn resubmit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, status)) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    if status != 2 {
        return Err(DomainError::Validation(
            "仅被拒种子可重提（当前状态不符）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, deny_reason_id = NULL, deny_note = NULL \
         WHERE id = $1 AND approval_status = 2",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("状态已变化，请刷新".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.resubmit", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "resubmitted": id })))
}

/// 删除种子（软删 approval_status=3；staff 任意删，作者仅限未过审）
#[delete("/torrents/{id}")]
async fn delete_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    torrents::delete_torrent(
        &state.repo.db,
        id,
        (auth.id, auth.class_id as i16),
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

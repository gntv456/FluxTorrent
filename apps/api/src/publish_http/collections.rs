//! 合集/系列（0157，阶段三聚合层）：collections(kind=collection|series) +
//! torrent_collections 多对多。与 0069 版本组互补——组是「一部作品多版本」，
//! 合集是「策展集合」（一种可入多个），系列是「作品序列」（组挂系列）。
//! 端点：建合集 / 收录种 / 列表 / 详情。管理权限：staff（class>=90）或
//! 合集创建者；收录操作与挂组同口径（种的 owner 或 staff）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CollectionCreateReq {
    name: String,
    /// collection（默认）| series
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    cover: Option<String>,
}

/// 建合集/系列（staff；同名同 kind 幂等返回已有）
#[post("/collections")]
pub async fn collection_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CollectionCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let name = body.name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(DomainError::Validation("合集名需 1-120 字".into()));
    }
    let kind = match body.kind.as_deref() {
        None | Some("collection") => "collection",
        Some("series") => "series",
        Some(_) => {
            return Err(DomainError::Validation(
                "kind 仅支持 collection/series".into(),
            ))
        }
    };
    if let Some(cid) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM collections WHERE kind = $1 AND name = $2",
    )
    .bind(kind)
    .bind(name)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    {
        return Ok(ok(serde_json::json!({ "id": cid, "existed": true })));
    }
    let cid: i64 = sqlx::query_scalar(
        "INSERT INTO collections (kind, name, descr, cover, created_by) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .bind(
        body.descr
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty()),
    )
    .bind(
        body.cover
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty()),
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "collection.create", Some(cid))
        .await;
    Ok(ok(serde_json::json!({ "id": cid })))
}

#[derive(Deserialize)]
pub struct CollectionAddReq {
    torrent_id: i64,
}

/// 收录种进合集（staff 或种的 owner；幂等）
#[post("/collections/{id}/items")]
pub async fn collection_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CollectionAddReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let cid = path.into_inner();
    let tid = body.torrent_id;
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(tid));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collections WHERE id = $1)",
    )
    .bind(cid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(cid));
    }
    sqlx::query(
        "INSERT INTO torrent_collections (collection_id, torrent_id, added_by) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(cid)
    .bind(tid)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "collection.add", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "added": true })))
}

/// 合集列表（kind 筛选可选；带收录数）
#[get("/collections")]
pub async fn collections_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<CollectionListQuery>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let kind = match q.kind.as_deref() {
        Some("collection") | Some("series") => q.kind.as_deref(),
        _ => None,
    };
    let rows: Vec<(i64, String, String, Option<String>, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT c.id, c.kind, c.name, c.descr, c.cover, \
                (SELECT count(*) FROM torrent_collections tc \
                 WHERE tc.collection_id = c.id) AS n \
             FROM collections c WHERE ($1::text IS NULL OR c.kind = $1) \
             ORDER BY c.sort_order, c.id LIMIT 200",
        )
        .bind(kind)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, k, name, descr, cover, n)| {
            serde_json::json!({
                "id": id, "kind": k, "name": name,
                "descr": descr, "cover": cover, "count": n,
            })
        })
        .collect();
    Ok(ok(items))
}

#[derive(Deserialize)]
struct CollectionListQuery {
    kind: Option<String>,
}

/// 合集详情：元数据 + 收录种摘要（复用列表行口径）
#[get("/collections/{id}")]
pub async fn collection_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let cid = path.into_inner();
    let meta: Option<(String, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT kind, name, descr, cover FROM collections WHERE id = $1",
        )
        .bind(cid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((kind, name, descr, cover)) = meta else {
        return Err(DomainError::NotFound(cid));
    };
    let rows: Vec<(i64, String, i64, i32, i32, i32)> = sqlx::query_as(
        "SELECT t.id, t.name, t.size, t.seeders, t.leechers, t.times_completed \
         FROM torrent_collections tc JOIN torrents t ON t.id = tc.torrent_id \
         WHERE tc.collection_id = $1 AND t.approval_status = 1 \
         ORDER BY tc.added_at DESC LIMIT 500",
    )
    .bind(cid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, n, size, s, l, c)| {
            serde_json::json!({
                "id": id, "name": n, "size": size,
                "seeders": s, "leechers": l, "times_completed": c,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "id": cid, "kind": kind, "name": name,
        "descr": descr, "cover": cover, "items": items,
    })))
}

/// 种子详情页「所属合集」数据源：该种被收录的全部合集
#[get("/torrents/{id}/collections")]
pub async fn torrent_collections(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    crate::torrents::assert_visible(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT c.id, c.kind, c.name, c.cover FROM torrent_collections tc \
         JOIN collections c ON c.id = tc.collection_id \
         WHERE tc.torrent_id = $1 ORDER BY c.id LIMIT 50",
    )
    .bind(tid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, kind, name, cover)| {
            serde_json::json!({
                "id": id, "kind": kind, "name": name, "cover": cover,
            })
        })
        .collect();
    Ok(ok(items))
}

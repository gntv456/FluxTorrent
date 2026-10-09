//! 自由值实体锚点（0327 music 批；0338 通用化）：列表 / 实体页 / 榜单。
//!
//! 承载决策见 0327/0338：artist / author / studio 都是低基数自由值实体，
//! 值仍存 `torrent_sections`（维度链路不动），本表只做「按名聚合的锚点」——
//! 聚合页/榜单/订阅的挂靠面。上传时的 upsert 同步在
//! `publish_http/upload_sections.rs::sync_content_anchors`。
//!
//! 0338 起 `kind` 参数化（`?kind=author` 即电子书作者页），前端复用同一套页面。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn norm_kind(v: Option<&str>) -> String {
    let k = v.unwrap_or("artist").trim().to_ascii_lowercase();
    if k.is_empty() {
        "artist".to_string()
    } else {
        k
    }
}

/// 实体列表（登录可读；搜索 + 分页）。
#[derive(Deserialize)]
struct ArtistQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
}

#[get("/artists")]
pub async fn artists_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ArtistQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = (q.page.unwrap_or(1).max(1) - 1) * limit;
    let kw = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let kind = norm_kind(q.kind.as_deref());
    // 该实体名下过审种数（值可能多人共用一条 sections 行）
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT a.id, a.name, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
          WHERE ts.kind = $4 \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%') \
         FROM artists a \
         WHERE a.kind = $4 \
           AND ($1::text IS NULL OR a.name ILIKE '%' || $1 || '%') \
         ORDER BY 3 DESC, a.norm_name LIMIT $2 OFFSET $3",
    )
    .bind(kw)
    .bind(limit)
    .bind(offset)
    .bind(&kind)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM artists WHERE kind = $2 \
           AND ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(&kw)
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(rows.len() as i64);
    Ok(ok(serde_json::json!({
        "kind": kind,
        "items": rows.iter().map(|(id, name, n)| serde_json::json!({
            "id": id, "name": name, "torrents": n,
        })).collect::<Vec<_>>(),
        "total": total,
    })))
}

/// 实体页：该实体名下全部过审种（时间倒序）。
#[get("/artists/{id}")]
pub async fn artist_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT name, kind FROM artists WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind)) = row else {
        return Err(DomainError::NotFound(id));
    };
    let items: Vec<(i64, String, i64, i32, i32)> = sqlx::query_as(
        "SELECT DISTINCT t.id, t.name, t.size, t.seeders, t.times_completed \
         FROM torrents t JOIN torrent_sections ts ON ts.torrent_id = t.id \
         WHERE ts.kind = $2 \
           AND ts.value #>> '{}' ILIKE '%' || $1 || '%' \
           AND t.approval_status = 1 \
         ORDER BY t.id DESC LIMIT 100",
    )
    .bind(&name)
    .bind(&kind)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id, "name": name, "kind": kind,
        "items": items.iter().map(|(tid, tn, sz, sd, tc)| serde_json::json!({
            "id": tid, "name": tn, "size": sz,
            "seeders": sd, "times_completed": tc,
        })).collect::<Vec<_>>(),
    })))
}

/// 榜单（按该实体名下总做种数）：Gazelle top artists 口径的最小实现。
#[get("/artists/top")]
pub async fn artists_top(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ArtistQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let kind = norm_kind(q.kind.as_deref());
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT a.id, a.name, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
          WHERE ts.kind = $1 \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%') AS releases, \
         (SELECT COALESCE(SUM(t.seeders), 0) FROM torrents t \
          JOIN torrent_sections ts ON ts.torrent_id = t.id \
          WHERE ts.kind = $1 \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%' \
            AND t.approval_status = 1) AS seeders \
         FROM artists a WHERE a.kind = $1 \
         ORDER BY seeders DESC, releases DESC, a.norm_name LIMIT 20",
    )
    .bind(&kind)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows
        .iter()
        .map(|(id, name, rel, sd)| {
            serde_json::json!({
                "id": id, "name": name,
                "releases": rel, "seeders": sd,
            })
        })
        .collect::<Vec<_>>()))
}

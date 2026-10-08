//! 艺人实体（0327 music 批）：列表 / 艺人页 / 榜单。
//!
//! 承载决策见迁移 0327：artist 是低基数字典型实体，值仍存
//! torrent_sections（维度链路不动），本表只做「按名聚合的锚点」——
//! 艺人页/榜单/订阅的挂靠面。上传时的 upsert 同步在 upload.rs
//! （发种成功路径，sections 落库后按 artist 值同步）。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 艺人列表（登录可读；搜索 + 分页）。
#[derive(Deserialize)]
struct ArtistQuery {
    #[serde(default)]
    q: Option<String>,
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
    // 该艺人名下过审种数与在档值数（artist 值可能多人共用一条 sections 行）
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT a.id, a.name, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
          WHERE ts.kind = 'artist' \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%') \
         FROM artists a \
         WHERE ($1::text IS NULL OR a.name ILIKE '%' || $1 || '%') \
         ORDER BY 3 DESC, a.norm_name LIMIT $2 OFFSET $3",
    )
    .bind(kw)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM artists \
         WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')",
    )
    .bind(&kw)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(rows.len() as i64);
    Ok(ok(serde_json::json!({
        "items": rows.iter().map(|(id, name, n)| serde_json::json!({
            "id": id, "name": name, "torrents": n,
        })).collect::<Vec<_>>(),
        "total": total,
    })))
}

/// 艺人页：该艺人名下全部过审种（时间倒序）。
#[get("/artists/{id}")]
pub async fn artist_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM artists WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(name) = name else {
        return Err(DomainError::NotFound(id));
    };
    let items: Vec<(i64, String, i64, i32, i32)> = sqlx::query_as(
        "SELECT DISTINCT t.id, t.name, t.size, t.seeders, t.times_completed \
         FROM torrents t JOIN torrent_sections ts ON ts.torrent_id = t.id \
         WHERE ts.kind = 'artist' \
           AND ts.value #>> '{}' ILIKE '%' || $1 || '%' \
           AND t.approval_status = 1 \
         ORDER BY t.id DESC LIMIT 100",
    )
    .bind(&name)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id, "name": name,
        "items": items.iter().map(|(tid, tn, sz, sd, tc)| serde_json::json!({
            "id": tid, "name": tn, "size": sz,
            "seeders": sd, "times_completed": tc,
        })).collect::<Vec<_>>(),
    })))
}

/// 艺人榜（按该艺人名下总做种数）：Gazelle top artists 口径的最小实现。
#[get("/artists/top")]
pub async fn artists_top(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT a.id, a.name, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
          WHERE ts.kind = 'artist' \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%') AS releases, \
         (SELECT COALESCE(SUM(t.seeders), 0) FROM torrents t \
          JOIN torrent_sections ts ON ts.torrent_id = t.id \
          WHERE ts.kind = 'artist' \
            AND ts.value #>> '{}' ILIKE '%' || a.name || '%' \
            AND t.approval_status = 1) AS seeders \
         FROM artists a \
         ORDER BY seeders DESC, releases DESC, a.norm_name LIMIT 20",
    )
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

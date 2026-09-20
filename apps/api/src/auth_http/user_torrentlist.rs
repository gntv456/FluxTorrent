//! 用户公开种子列表（M01）：GET /users/{id}/torrentlist。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::http::SnatchRow;
use crate::state::AppState;

#[get("/users/{id}/torrentlist")]
pub async fn user_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<UserTorrentlistQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = path.into_inner();
    // 隐私红线（P1）：做种/下载/完成明细可反推用户下载偏好，仅本人与 staff 可见
    // （NP userdetails 的普通用户口径）。uploads 本就过滤匿名发布、preserved 是
    // 保种区的公开认领承诺，保持公开。
    let reveal = auth.id == uid || auth.class_id >= 90;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let uploads: Vec<SnatchRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
         false AS seeding, false AS leeching, NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here \
         FROM torrents t WHERE t.owner_id = $1 AND t.approval_status = 1 AND NOT t.anonymous \
         ORDER BY t.id DESC LIMIT $2",
    )
    .bind(uid)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let seeding: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    // —— 憨憨式标签页其余四路（NP userdetails 同口径）——
    // 当前下载：leeching 抓取记录；完成：completed_at 非空；未完成：抓过但未完成且已不在做种/下载
    let leeching: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.leeching \
             ORDER BY s.torrent_id DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    let completed: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    let incomplete: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NULL AND NOT s.seeding AND NOT s.leeching \
             ORDER BY s.last_seen_at DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    // 完成的保种区种子：seed_preserve 中该用户认领且尚未退出的记录
    // （认领本身公开；snatch 派生字段只对本人/staff 展示）
    let preserved: Vec<SnatchRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
         CASE WHEN $3 THEN s.seeding ELSE false END AS seeding, \
         CASE WHEN $3 THEN s.leeching ELSE false END AS leeching, \
         CASE WHEN $3 THEN s.completed_at END AS completed_at, \
         CASE WHEN $3 THEN s.uploaded ELSE 0 END AS uploaded_here \
         FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         LEFT JOIN snatches s ON s.torrent_id = sp.torrent_id AND s.user_id = $1 \
         WHERE sp.claimed_by = $1 AND sp.exited_at IS NULL \
         ORDER BY sp.torrent_id DESC LIMIT $2",
    )
    .bind(uid)
    .bind(limit)
    .bind(reveal)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "uploads": uploads, "seeding": seeding, "leeching": leeching,
        "completed": completed, "incomplete": incomplete, "preserved": preserved,
    })))
}

#[derive(Deserialize)]
pub(super) struct UserTorrentlistQuery {
    #[serde(default)]
    pub(super) limit: Option<i64>,
}

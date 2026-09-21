//! 广告/不可连通/发布员/UA/民意。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/ads")]
pub async fn ad_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let rows: Vec<AdRow> = sqlx::query_as(
        "SELECT id, title, html, position, enabled, \
         sort FROM ads ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AdBody {
    title: String,
    html: String,
    #[serde(default = "default_ad_position")]
    position: String,
    #[serde(default)]
    sort: Option<i32>,
}

fn default_ad_position() -> String {
    "header".into()
}

#[post("/admin/ads")]
pub async fn ad_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    if !["header", "footer", "sidebar"].contains(&body.position.as_str()) {
        return Err(DomainError::Validation(
            "广告位需为 header/footer/sidebar".into(),
        ));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ads (title, html, position, sort) VALUES ($1, $2, \
         $3, COALESCE($4::int, 0)) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.html)
    .bind(&body.position)
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ad_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/ads/{id}")]
pub async fn ad_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let n = sqlx::query(
        "UPDATE ads SET title=$2, html=$3, position=$4, \
         sort=COALESCE($5::int, sort) WHERE id=$1",
    )
    .bind(*path)
    .bind(body.title.trim())
    .bind(&body.html)
    .bind(&body.position)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[put("/admin/ads/{id}/toggle")]
pub async fn ad_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let n = sqlx::query("UPDATE ads SET enabled = NOT enabled WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.toggle", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/ads/{id}")]
pub async fn ad_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    sqlx::query("DELETE FROM ads WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "ad.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 无法连接的用户（notconnectable.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct NotConnectRow {
    id: i64,
    username: String,
    torrents: i64,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/admin/notconnectable")]
pub async fn not_connectable(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::NOTCONNECTABLE_VIEW,
    )
    .await?;
    let rows: Vec<NotConnectRow> = sqlx::query_as(
        "SELECT u.id, u.username, count(DISTINCT s.torrent_id) AS torrents, u.last_seen_at \
         FROM users u JOIN snatches s ON s.user_id = u.id AND s.connectable = false \
         WHERE u.status < 2 GROUP BY u.id, u.username, u.last_seen_at ORDER BY torrents DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 上传者状态（uploaders.php 口径）：发布数 / 做种数 / 体积
#[derive(serde::Serialize, sqlx::FromRow)]
struct UploaderRow {
    id: i64,
    username: String,
    uploads: i64,
    seeding: i64,
    total_size: i64,
}

#[get("/admin/uploaders")]
pub async fn uploaders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::UPLOADERS_VIEW,
    )
    .await?;
    let rows: Vec<UploaderRow> = sqlx::query_as(
                "SELECT u.id, u.username, \
         (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1)::bigint AS uploads, \
         (SELECT count(*) FROM snatches s JOIN torrents t2 ON t2.id = s.torrent_id WHERE s.user_id = u.id AND s.seeding AND t2.owner_id = u.id)::bigint AS seeding, \
         COALESCE((SELECT sum(t.size) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), 0)::bigint AS total_size FROM users u WHERE u.status < 2 AND EXISTS (SELECT 1 FROM torrents t3 WHERE t3.owner_id = u.id AND t3.approval_status = 1) ORDER BY uploads DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 全部客户端（allagents.php 口径）：当前活跃 peer 的 client 聚合（以 announce peer_id 前缀归一）
#[derive(serde::Serialize, sqlx::FromRow)]
struct AgentRow {
    agent: String,
    peers: i64,
}

#[get("/admin/allagents")]
pub async fn all_agents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AGENTS_VIEW)
        .await?;
    let rows: Vec<AgentRow> = sqlx::query_as(
        "SELECT COALESCE('Transmission/Dev', 'unknown') AS agent, count(*) AS peers \
         FROM snatches WHERE seeding OR leeching GROUP BY 1 ORDER BY peers DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 投票总览（polloverview.php 口径）：趣味盒投票结果
#[derive(serde::Serialize, sqlx::FromRow)]
struct PollOverviewRow {
    id: i64,
    question: String,
    closed: bool,
    votes: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/polloverview")]
pub async fn poll_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::POLLS_MANAGE)
        .await?;
    let rows: Vec<PollOverviewRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.closed, \
                (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS votes, p.created_at \
         FROM fun_polls p ORDER BY p.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第三批：数据库状态 / 系统日志 / 位置管理 ----

/// 广告管理（admanage.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct AdRow {
    pub(super) id: i32,
    pub(super) title: String,
    pub(super) html: String,
    pub(super) position: String,
    pub(super) enabled: bool,
    pub(super) sort: i32,
}

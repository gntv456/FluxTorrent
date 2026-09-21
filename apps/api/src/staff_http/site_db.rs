//! DB 统计/系统日志/选址。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/dbstats")]
pub async fn db_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::DBSTATS_VIEW)
        .await?;
    let conns: Vec<PgConnRow> = sqlx::query_as(
                "SELECT state, \
         count(*)::bigint AS count FROM pg_stat_activity WHERE datname = current_database() GROUP BY state ORDER BY count DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let db_size: i64 = sqlx::query_scalar(
        "SELECT pg_database_size(current_database())::bigint",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let tables: Vec<TableSizeRow> = sqlx::query_as(
                "SELECT c.relname, \
         pg_total_relation_size(c.oid)::bigint AS total_size, \
         c.reltuples::bigint AS row_estimates FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relkind = 'r' ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 15",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let slow_tx: i64 = sqlx::query_scalar(
                "SELECT count(*)::bigint FROM pg_stat_activity WHERE datname = \
         current_database() AND xact_start IS NOT NULL AND now() - xact_start > \
         interval '30 seconds'",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    let dead_tuples: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(n_dead_tup), 0)::bigint FROM pg_stat_user_tables",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let total_conns: i64 = conns.iter().map(|c| c.count).sum();
    Ok(ok(serde_json::json!({
        "engine": "PostgreSQL",
        "database": "fluxtorrent",
        "connections": conns,
        "total_connections": total_conns,
        "database_size": db_size,
        "slow_transactions": slow_tx,
        "dead_tuples": dead_tuples,
        "tables": tables,
    })))
}

/// 系统日志（bitbucketlog.php 口径 → 审计日志分页）
#[derive(serde::Serialize, sqlx::FromRow)]
struct SysLogRow {
    id: i64,
    actor: Option<String>,
    action: String,
    ref_json: Option<serde_json::Value>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct SysLogQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    q: Option<String>,
}

#[get("/admin/syslog")]
pub async fn sys_log(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SysLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SYSLOG_VIEW)
        .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<SysLogRow> = sqlx::query_as(
                "SELECT l.id, u.username AS actor, l.action, \
         l.ref AS ref_json, host(l.ip) AS ip, \
         l.created_at FROM audit_log l LEFT JOIN users u ON u.id = l.actor_id WHERE ($1::text IS NULL OR l.action ILIKE '%' || $1 || '%') ORDER BY l.id DESC LIMIT $2 OFFSET $3",
    ).bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log l WHERE ($1::text IS NULL OR \
         l.action ILIKE '%' || $1 || '%')",
    )
    .bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

/// 位置管理（location.php 口径 → login_events 按 IP 网段归组的位置视图）
/// Dev 环境无 GeoIP 库：IPv4 以 /24 网段、IPv6 以 /64 网段为位置单元聚合
#[derive(serde::Serialize, sqlx::FromRow)]
struct LocationRow {
    net: String,
    netmask: i32,
    logins: i64,
    users: i64,
    failed: i64,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct LocationQuery {
    #[serde(default)]
    page: Option<i64>,
}

#[get("/admin/locations")]
pub async fn locations(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LocationQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::LOCATIONS_MANAGE,
    )
    .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<LocationRow> = sqlx::query_as(
                "SELECT host(n.net) AS net, masklen(n.net) AS netmask, \
         n.logins, n.users, n.failed, \
         n.last_seen FROM ( SELECT (CASE family(ip) WHEN 4 THEN network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END) AS net, count(*)::bigint AS logins, count(DISTINCT user_id)::bigint AS users, count(*) FILTER (WHERE NOT ok)::bigint AS failed, max(created_at) AS last_seen FROM login_events WHERE ip IS NOT NULL GROUP BY 1 ) n ORDER BY n.last_seen DESC NULLS LAST LIMIT $1 OFFSET $2",
    ).bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ( SELECT (CASE family(ip) WHEN 4 THEN \
         network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END) \
         FROM login_events WHERE ip IS NOT NULL GROUP BY 1 ) t",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

// ---- 通用 PT 站点类型系统（site-type packs：教育/影视/音乐/…可切换）----

/// 数据库状态（mysql_stats.php 口径 → PostgreSQL）：连接数/库大小/表大小 Top/长事务
#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct PgConnRow {
    pub(super) state: String,
    pub(super) count: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct TableSizeRow {
    pub(super) relname: String,
    pub(super) total_size: i64,
    pub(super) row_estimates: i64,
}

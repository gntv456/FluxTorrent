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

/// 运行日志（0218 G6）：api/worker 的 WARN+ 事件出口。
/// 此前这里读 audit_log —— 那是「谁做了什么」的操作审计（在 ?tool=audit 页），
/// 站点报错时站长查不到任何运行痕迹。现改读 runtime_logs（两级 tracing 层落库，
/// 见 apps/api/src/runtime_log.rs 与 apps/worker/src/runtime_log.rs）。
#[derive(serde::Serialize, sqlx::FromRow)]
struct RuntimeLogRow {
    id: i64,
    ts: chrono::DateTime<chrono::Utc>,
    level: String,
    source: String,
    target: String,
    message: String,
    repeat: i32,
    instance: String,
}

#[derive(Deserialize)]
struct RuntimeLogQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    q: Option<String>,
    /// 级别筛选：ERROR / WARN（空 = 全部）
    #[serde(default)]
    level: Option<String>,
    /// 实例筛选（0224 G30）：多副本下按 FLUX_INSTANCE_ID / 容器 hostname 过滤
    #[serde(default)]
    instance: Option<String>,
}

#[get("/admin/syslog")]
pub async fn sys_log(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RuntimeLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SYSLOG_VIEW)
        .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 50i64;
    let kw = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let lv = q.level.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let inst =
        q.instance.as_deref().map(str::trim).filter(|s| !s.is_empty());
    // 列表与计数同一谓词（_doc/同类BUG排查报告-20260916.md 的口径纪律）
    let where_sql = "($1::text IS NULL OR level = $1) \
         AND ($2::text IS NULL OR message ILIKE '%' || $2 || '%' \
              OR target ILIKE '%' || $2 || '%') \
         AND ($5::text IS NULL OR instance = $5)";
    let rows: Vec<RuntimeLogRow> = sqlx::query_as(&format!(
        "SELECT id, ts, level, source, target, message, repeat, instance \
         FROM runtime_logs WHERE {where_sql} \
         ORDER BY id DESC LIMIT $3 OFFSET $4",
    ))
    .bind(lv)
    .bind(kw)
    .bind(per)
    .bind((page - 1) * per)
    .bind(inst)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM runtime_logs WHERE {where_sql}",
    ))
    .bind(lv)
    .bind(kw)
    .bind(inst)
    .bind((page - 1) * per)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 24h 级别计数：面板顶部常显「近期告警/错误」，不必翻页找
    let counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT level, count(*)::bigint FROM runtime_logs \
         WHERE ts > now() - interval '24 hours' \
         GROUP BY level ORDER BY level",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    // 实例下拉（0224 G30）：只列近 7 天出现过的实例，多副本部署时区分机器；
    // 单实例（instance=''）时列表为空、前端隐藏筛选器
    let instances: Vec<(String, i64)> = sqlx::query_as(
        "SELECT instance, count(*)::bigint FROM runtime_logs \
         WHERE ts > now() - interval '7 days' AND instance <> '' \
         GROUP BY instance ORDER BY count(*) DESC LIMIT 20",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
        "counts_24h": counts,
        "instances": instances,
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

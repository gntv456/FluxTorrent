//! v4 缺口批次（0079）：api 侧 Prometheus /metrics、火花日度对账、商店权益效果、
//! 成就四族（worker 侧扫描在 jobs.rs）、POSTPONED 审核第四态、规则页版本化。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ G3：api /metrics（Prometheus 文本；token 门禁同 tracker 口径） ============

/// 全进程请求计数（path 维度聚合到方法级，防标签爆炸）
pub static METRIC_REQUESTS: AtomicU64 = AtomicU64::new(0);
pub static METRIC_REQUESTS_5XX: AtomicU64 = AtomicU64::new(0);

/// 请求计数中间件（from_fn 包装；计数是尽力而为，不影响请求路径）
pub async fn metrics_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    METRIC_REQUESTS.fetch_add(1, Ordering::Relaxed);
    let res = next.call(req).await?;
    if res.status().as_u16() >= 500 {
        METRIC_REQUESTS_5XX.fetch_add(1, Ordering::Relaxed);
    }
    Ok(res)
}

#[get("/metrics")]
async fn api_metrics(req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>) -> HttpResponse {
    // 与 tracker /metrics 同款门禁：未配 token 时 404 不暴露
    let tok = std::env::var("API_METRICS_TOKEN").unwrap_or_default();
    if tok.is_empty()
        || req
            .headers()
            .get("x-metrics-token")
            .and_then(|v| v.to_str().ok())
            != Some(tok.as_str())
    {
        return HttpResponse::NotFound().finish();
    }
    // 池状态是 api 侧最有价值的 gauge（容量规划的底线数据）
    let pool = &state.repo.db;
    let pool_size = pool.size();
    let pool_idle = pool.num_idle();
    let body = format!(
        concat!(
            "# HELP flux_api_requests_total Total requests since start.\n",
            "# TYPE flux_api_requests_total counter\n",
            "flux_api_requests_total {}\n",
            "# HELP flux_api_requests_5xx_total Responses with status >= 500.\n",
            "# TYPE flux_api_requests_5xx_total counter\n",
            "flux_api_requests_5xx_total {}\n",
            "# HELP flux_api_pool_connections Current PG pool connections.\n",
            "# TYPE flux_api_pool_connections gauge\n",
            "flux_api_pool_connections {}\n",
            "# HELP flux_api_pool_idle Idle PG pool connections.\n",
            "# TYPE flux_api_pool_idle gauge\n",
            "flux_api_pool_idle {}\n"
        ),
        METRIC_REQUESTS.load(Ordering::Relaxed),
        METRIC_REQUESTS_5XX.load(Ordering::Relaxed),
        pool_size,
        pool_idle,
    );
    HttpResponse::Ok().content_type("text/plain").body(body)
}

// ============ G17：火花日度对账（净增率常驻仪表数据源） ============

#[get("/admin/spark-flow/daily")]
async fn spark_flow_daily(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_VIEW).await?;
    let rows: Vec<(chrono::NaiveDate, i64, i64, i64)> = sqlx::query_as(
        "SELECT day, minted::bigint, burned::bigint, net::bigint FROM v_spark_flow_daily",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ G6：我的成就 + 全站成就定义 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AchievementRow {
    code: String,
    family: String,
    name: String,
    descr: String,
    threshold: i64,
    reward_sparks: i64,
    earned: bool,
    granted_at: Option<chrono::DateTime<chrono::Utc>>,
    metric_value: Option<i64>,
}

#[get("/me/achievements")]
async fn my_achievements(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, AchievementRow>(
        "SELECT d.code, d.family, d.name, d.descr, d.threshold, d.reward_sparks, \
                (ua.user_id IS NOT NULL) AS earned, ua.granted_at, ua.metric_value \
         FROM achievement_defs d \
         LEFT JOIN user_achievements ua ON ua.def_id = d.id AND ua.user_id = $1 \
         ORDER BY d.family, d.position",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ G7：POSTPONED 审核第四态（U3D 口径） ============
// approval_status 语义扩容：0待审 1过审 2被拒 3下架 4暂缓。
// 暂缓种仅发布者本人与 staff 可见；证据补齐后走常规 approve/reject 决策。

#[derive(Deserialize)]
struct PostponeReq {
    torrent_id: i64,
    #[serde(default)]
    reason: String,
}

/// 暂缓（staff）：待审/被拒 → 暂缓（证据不足挂起）
#[post("/admin/reviews/postpone")]
async fn review_postpone(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostponeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TORRENT_MANAGE).await?;
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 4, deny_note = COALESCE(NULLIF($2,''), deny_note) \
         WHERE id = $1 AND approval_status IN (0, 2)",
    )
    .bind(body.torrent_id)
    .bind(body.reason.trim())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在可暂缓状态".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "review.postpone", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "id": body.torrent_id, "status": 4 })))
}

/// 暂缓恢复（staff）：暂缓 → 待审（回到常规审核流；approve/reject 仍走既有 decide）
#[post("/admin/reviews/resume")]
async fn review_resume(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostponeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TORRENT_MANAGE).await?;
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, deny_note = NULL WHERE id = $1 AND approval_status = 4",
    )
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在暂缓状态".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "review.resume", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "id": body.torrent_id, "status": 0 })))
}

// ============ G5：规则页版本化（修订历史；Gazelle Wiki revision 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct RuleRevisionRow {
    id: i64,
    rule_id: i32,
    title: String,
    edited_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 某规则的修订历史（staff；正文含在列表里，供对照）
#[get("/admin/rules/{id}/revisions")]
async fn rule_revisions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE).await?;
    let rows: Vec<(i64, i32, String, String, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT r.id, r.rule_id, r.title, r.body, u.username, r.created_at \
             FROM rules_revisions r LEFT JOIN users u ON u.id = r.edited_by \
             WHERE r.rule_id = $1 ORDER BY r.created_at DESC LIMIT 50",
        )
        .bind(*path)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

pub fn mount_v4(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(api_metrics)
        .service(spark_flow_daily)
        .service(my_achievements)
        .service(review_postpone)
        .service(review_resume)
        .service(rule_revisions)
}

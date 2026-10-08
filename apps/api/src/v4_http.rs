//! v4 缺口批次（0079）：api 侧 Prometheus /metrics、火花日度对账、商店权益效果、
//! 成就四族（worker 侧扫描在 jobs.rs）、POSTPONED 审核第四态、规则页版本化。

mod metrics;

pub use metrics::metrics_mw;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ G17：火花日度对账（净增率常驻仪表数据源） ============

#[get("/admin/spark-flow/daily")]
async fn spark_flow_daily(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(chrono::NaiveDate, i64, i64, i64)> = sqlx::query_as(
        "SELECT day, minted::bigint, burned::bigint, \
         net::bigint FROM v_spark_flow_daily",
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
) -> DomainResult<impl actix_web::Responder> {
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
    // 审计 2026-10-08 P2-4：暂缓与裁决同权（旧口径 torrent.manage，收窄
    // review 权后暂缓口反而更大）；并补自审拦截，与 decide 同口径。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(body.torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(owner_id) = owner_id else {
        return Err(DomainError::NotFound(body.torrent_id));
    };
    if owner_id == auth.id {
        return Err(DomainError::Validation("不能审核自己发布的种子".into()));
    }
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
        return Err(DomainError::Validation(
            "种子不存在或不在可暂缓状态".into(),
        ));
    }
    // 与 decide 同口径留痕（P2-4：旧版只写 audit_log，操作时间线缺这一环）
    let _ = sqlx::query(
        "INSERT INTO torrent_operation_logs (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, 'postpone', $3)",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(serde_json::json!({ "reason": body.reason.trim() }).to_string())
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "review.postpone", Some(body.torrent_id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": body.torrent_id, "status": 4 }),
    ))
}

/// 暂缓恢复（staff）：暂缓 → 待审（回到常规审核流；approve/reject 仍走既有 decide）
#[post("/admin/reviews/resume")]
async fn review_resume(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostponeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 与 postpone 同权（P2-4：resume 的对手面是暂缓操作者，同样是审核动作）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, \
         deny_note = NULL WHERE id = $1 AND approval_status = 4",
    )
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在暂缓状态".into()));
    }
    // 与 decide 同口径留痕（P2-4）
    let _ = sqlx::query(
        "INSERT INTO torrent_operation_logs (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, 'resume', '{}')",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "review.resume", Some(body.torrent_id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": body.torrent_id, "status": 0 }),
    ))
}

// ============ G5：规则页版本化（修订历史；Gazelle Wiki revision 口径） ============

/// 某规则的修订历史（staff；正文含在列表里，供对照）
#[get("/admin/rules/{id}/revisions")]
async fn rule_revisions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    let rows: Vec<(
        i64,
        i32,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
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
        .service(metrics::api_metrics)
        .service(spark_flow_daily)
        .service(my_achievements)
        .service(review_postpone)
        .service(review_resume)
        .service(rule_revisions)
}

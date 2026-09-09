//! M21 绩效考核 + M22 任务中心 + M19 保种区前台 HTTP 接口。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_ops(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M21 绩效考核
        .service(jixiao_types)
        .service(jixiao_claim)
        .service(jixiao_my)
        // M22 任务中心
        .service(task_list)
        .service(task_claim)
        // M19 保种区
        .service(preserve_list)
        .service(preserve_claim)
}

// ============ M21 绩效考核 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct JixiaoTypeRow {
    id: i64,
    name: String,
    base_pay: i64,
    metrics: serde_json::Value,
    min_requirements: serde_json::Value,
}

#[get("/jixiao/types")]
async fn jixiao_types(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, JixiaoTypeRow>(
        "SELECT id, name, base_pay, metrics, min_requirements FROM jixiao_types ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 指标采集：全部来自系统流水（announce 统计/保种表/操作日志），零手工填报（M21 验收）
async fn compute_metrics(
    db: &sqlx::PgPool,
    user_id: i64,
    period: &str, // "2026-09"
) -> DomainResult<serde_json::Value> {
    let record: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT \
            COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint, \
            COALESCE((SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id = $1 AND seeding),0)::bigint, \
            COALESCE((SELECT count(*) FROM audit_log WHERE actor_id = $1),0)::bigint \
         FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up, down, seeding_count, ops) = record.unwrap_or((0, 0, 0, 0));
    let uploads: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size),0)::bigint FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 AND s.seeding",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "uploaded": up, "downloaded": down, "uploads": uploads,
        "seeding_count": seeding_count, "seed_size": seed_size, "ops": ops,
    }))
}

#[derive(Deserialize)]
struct ClaimReq {
    type_id: i64,
    #[serde(default)]
    period: Option<String>,
}

#[post("/jixiao/claim")]
async fn jixiao_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let period = body
        .period
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());

    // 幂等：本期已领（UNIQUE 约束兜底 + 先查友好报错）
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND period = $3)",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if claimed {
        return Err(DomainError::Validation("本期已领取过工资".into()));
    }

    let t: Option<(String, i64, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT name, base_pay, metrics, min_requirements FROM jixiao_types WHERE id = $1",
    )
    .bind(body.type_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, base_pay, _metrics, min_reqs)) = t else {
        return Err(DomainError::NotFound(body.type_id));
    };

    let metrics = compute_metrics(&state.repo.db, auth.id, &period).await?;
    // 最低要求线校验（低于要求不发基本工资 —— 旧站口径）
    let m = &metrics;
    for (key, min) in min_reqs.as_object().unwrap_or(&serde_json::Map::new()) {
        let actual = m.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let required = min.as_i64().unwrap_or(0);
        if required > 0 && actual < required {
            return Err(DomainError::Validation(format!(
                "「{name}」考核未达标：{key} 需 {required}，实际 {actual}"
            )));
        }
    }
    // 达标月数加成：每累计 3 个达标月 +10%
    let qualified_months: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let bonus = base_pay / 10 * (qualified_months / 3);
    let total = base_pay + bonus;

    let claim_id: i64 = sqlx::query_scalar(
        "INSERT INTO jixiao_claims (user_id, type_id, period, amount, metrics_snapshot) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .bind(total)
    .bind(metrics.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("本期已领取过工资".into())
        }
        other => DomainError::Internal(other.into()),
    })?;

    let idem = format!("jixiao:{}:{}", claim_id, period);
    earn_spark(&state.repo.db, auth.id, total, "task_reward", &idem).await?;
    state
        .repo
        .audit(Some(auth.id), "jixiao_claim", Some(claim_id))
        .await;
    Ok(ok(
        serde_json::json!({ "claim_id": claim_id, "type": name, "base": base_pay, "bonus": bonus, "total": total }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ClaimRow {
    id: i64,
    type_name: String,
    period: String,
    amount: i64,
    claimed_at: chrono::DateTime<chrono::Utc>,
}

#[get("/jixiao/my")]
async fn jixiao_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.id, t.name AS type_name, c.period, c.amount, c.claimed_at \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.user_id = $1 ORDER BY c.id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M22 任务中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TaskRow {
    id: i64,
    name: String,
    metric: serde_json::Value,
    reward: i64,
    penalty: i64,
    claim_limit: Option<i32>,
    claimed: i64,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
}

#[get("/tasks")]
async fn task_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let uid = require_auth(&req, &state).await.ok().map(|a| a.id);
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT t.id, t.name, t.metric, t.reward, t.penalty, t.claim_limit, \
            (SELECT count(*) FROM task_claims tc WHERE tc.task_id = t.id)::bigint AS claimed, \
            t.starts_at, t.ends_at \
         FROM tasks t WHERE now() BETWEEN t.starts_at AND t.ends_at \
         ORDER BY t.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TaskClaimReq {
    task_id: i64,
}

#[post("/tasks/claim")]
async fn task_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TaskClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 认领人数限流（旧站 23/100 口径 → 数据库计数 + 唯一约束）
    let limit: Option<i32> = sqlx::query_scalar("SELECT claim_limit FROM tasks WHERE id = $1")
        .bind(body.task_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(limit) = limit else {
        return Err(DomainError::NotFound(body.task_id));
    };
    if limit > 0 {
        let claimed: i64 =
            sqlx::query_scalar("SELECT count(*) FROM task_claims WHERE task_id = $1")
                .bind(body.task_id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(0);
        if claimed >= limit as i64 {
            return Err(DomainError::Validation("认领名额已满".into()));
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO task_claims (task_id, user_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(body.task_id)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("已认领过该任务".into())
        }
        other => DomainError::Internal(other.into()),
    })?;
    Ok(ok(serde_json::json!({ "claim_id": id })))
}

// ============ M19 保种区 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PreserveRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    claimed_by: Option<String>,
}

#[get("/preserve")]
async fn preserve_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, PreserveRow>(
        "SELECT sp.torrent_id, t.name, t.size, t.seeders, u.username AS claimed_by \
         FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         LEFT JOIN users u ON u.id = sp.claimed_by \
         WHERE sp.exited_at IS NULL AND t.approval_status = 1 \
         ORDER BY t.seeders ASC, sp.torrent_id LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct PreserveClaimReq {
    torrent_id: i64,
}

#[post("/preserve/claim")]
async fn preserve_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PreserveClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let updated = sqlx::query(
        "UPDATE seed_preserve SET claimed_by = $2, claimed_at = now() \
         WHERE torrent_id = $1 AND claimed_by IS NULL",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation("该种子已被认领或不在保种区".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "preserve_claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.torrent_id })))
}

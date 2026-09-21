//! P3-13 考核岗位类型 CRUD
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P3-13 考核岗位类型 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct JixiaoTypeRow {
    id: i64,
    name: String,
    metrics: serde_json::Value,
    base_pay: i64,
    min_requirements: serde_json::Value,
    bonus_rules: serde_json::Value,
    #[sqlx(default)]
    description: String,
    /// 本期登记人数（列表「登记数」列真实数据，旧版硬编码 "—"）
    #[sqlx(default)]
    assigned_count: i64,
}

#[get("/admin/jixiao-types")]
async fn jixiao_types_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let period = (chrono::Utc::now() + chrono::Duration::hours(8))
        .format("%Y-%m")
        .to_string();
    let rows: Vec<JixiaoTypeRow> = sqlx::query_as(
        "SELECT t.id, t.name, t.metrics, t.base_pay, t.min_requirements, t.bonus_rules, t.description, \
                (SELECT count(*) FROM jixiao_claims c \
                 WHERE c.type_id = t.id AND c.period = $1 AND c.metrics_snapshot->>'source' = 'admin') AS assigned_count \
         FROM jixiao_types t ORDER BY t.id",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
pub(crate) struct JixiaoTypeReq {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) base_pay: Option<i64>,
    #[serde(default)]
    pub(crate) min_requirements: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) bonus_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) description: Option<String>,
}

/// 死键防线（0106）：min_requirements/metrics 出现白名单外的键直接拒绝保存。
/// 旧实现没这层校验，种子数据里 seed_days/seed_hours/seed_size_tb 配进去后
/// 达标判定永远失败（compute_metrics 不产出这些键）。
pub(super) fn jixiao_reject_unknown_keys(
    body: &JixiaoTypeReq,
) -> DomainResult<()> {
    for field in [&body.min_requirements, &body.metrics] {
        if let Some(v) = field {
            if let Some(k) = crate::ops_http::jixiao_unknown_metric_key(v) {
                return Err(DomainError::Validation(format!(
                    "未知指标键「{k}」，可用键：uploaded/downloaded/uploads/seeding_count/\
                     seed_size/seed_size_tb/seed_hours/avg_seed_hours/seed_days/\
                     seed_points_delta/spark_delta/ops"
                )));
            }
        }
    }
    Ok(())
}

#[post("/admin/jixiao-types")]
async fn jixiao_type_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JixiaoTypeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("岗位名不能为空".into()));
    }
    jixiao_reject_unknown_keys(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO jixiao_types (name, metrics, base_pay, min_requirements, bonus_rules, description) \
         VALUES ($1, COALESCE($2, '{}'::jsonb), COALESCE($3, 0), COALESCE($4, '{}'::jsonb), COALESCE($5, '{}'::jsonb), COALESCE($6, '')) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.metrics.clone())
    .bind(body.base_pay)
    .bind(body.min_requirements.clone())
    .bind(body.bonus_rules.clone())
    .bind(body.description.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/jixiao-types/{id}")]
async fn jixiao_type_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<JixiaoTypeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    jixiao_reject_unknown_keys(&body)?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE jixiao_types SET name = $2, metrics = COALESCE($3, metrics), base_pay = COALESCE($4, base_pay), \
           min_requirements = COALESCE($5, min_requirements), bonus_rules = COALESCE($6, bonus_rules), \
           description = COALESCE($7, description) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.metrics.clone())
    .bind(body.base_pay)
    .bind(body.min_requirements.clone())
    .bind(body.bonus_rules.clone())
    .bind(body.description.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/jixiao-types/{id}")]
async fn jixiao_type_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    let id = path.into_inner();
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jixiao_claims WHERE type_id = $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation(
            "已有考核登记引用该岗位，不可删除".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM jixiao_types WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.del", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

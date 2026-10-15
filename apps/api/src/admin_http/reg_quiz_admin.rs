//! 注册答题题库 admin API（0372 M5）：列表 / 新增 / 更新（门禁 300 行，
//! 校验、导入、删除在 reg_quiz_bank.rs）。
//!   GET  /admin/reg-quiz            题库列表 + 近 N 天通过率
//!   POST /admin/reg-quiz            新增一题
//!   POST /admin/reg-quiz/update     改题干/选项/分类/启用态
//!
//! 出题口径：单选；options 为字符串数组（2-6 项），answer_index 0 基下标。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
use super::reg_quiz_bank::{validate_quiz, QuizReq};

#[derive(sqlx::FromRow, serde::Serialize)]
struct QuizRow {
    id: i64,
    category: String,
    question: String,
    options: serde_json::Value,
    answer_index: i32,
    explanation: Option<String>,
    enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct QuizQuery {
    limit: Option<i64>,
}

/// GET /admin/reg-quiz：题库 + 近 N 天答题通过率。
#[get("/admin/reg-quiz")]
async fn reg_quiz_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<QuizQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    let rows: Vec<QuizRow> = sqlx::query_as(
        "SELECT id, category, question, options, answer_index, \
                explanation, enabled, created_at \
         FROM reg_quiz_questions ORDER BY enabled DESC, category, id \
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'cheat_accuracy_days'), 90)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(90)
    .clamp(1, 3650);
    let (attempts, passed): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE passed) \
         FROM reg_quiz_attempts \
         WHERE created_at > now() - ($1::bigint * interval '1 day')",
    )
    .bind(days)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or((0, 0));
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM reg_quiz_questions")
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "stats": {
            "days": days,
            "attempts": attempts,
            "passed": passed,
            "pass_rate": if attempts > 0 {
                (passed as f64) / (attempts as f64)
            } else { 0.0 },
        },
    }))))
}

/// POST /admin/reg-quiz：新增一题。
#[post("/admin/reg-quiz")]
async fn reg_quiz_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<QuizReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let (cat, question, opts, ans, expl) =
        validate_quiz(&body).map_err(DomainError::Validation)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO reg_quiz_questions \
           (category, question, options, answer_index, explanation, \
            enabled, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(&cat)
    .bind(&question)
    .bind(serde_json::Value::from(opts))
    .bind(ans)
    .bind(&expl)
    .bind(body.enabled.unwrap_or(true))
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "reg_quiz.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct QuizUpdate {
    id: i64,
    #[serde(default)]
    enabled: Option<bool>,
    /// 有非空题干 = 整题重写（须同时给 options + answer_index）
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    options: Vec<String>,
    #[serde(default)]
    answer_index: Option<i32>,
    #[serde(default)]
    explanation: Option<String>,
}

/// POST /admin/reg-quiz/update：改启用态（最常用）或整题重写。
#[post("/admin/reg-quiz/update")]
async fn reg_quiz_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<QuizUpdate>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let has_body = body
        .question
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let n = if has_body {
        let req_q = QuizReq {
            category: body.category.clone().unwrap_or_default(),
            question: body.question.clone().unwrap_or_default(),
            options: body.options.clone(),
            answer_index: body.answer_index.unwrap_or(-1),
            explanation: body.explanation.clone(),
            enabled: body.enabled,
        };
        let (cat, question, opts, ans, expl) =
            validate_quiz(&req_q).map_err(DomainError::Validation)?;
        sqlx::query(
            "UPDATE reg_quiz_questions SET category = $1, question = $2, \
               options = $3, answer_index = $4, explanation = $5, \
               enabled = COALESCE($6, enabled) WHERE id = $7",
        )
        .bind(&cat)
        .bind(&question)
        .bind(serde_json::Value::from(opts))
        .bind(ans)
        .bind(&expl)
        .bind(body.enabled)
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    } else {
        let Some(en) = body.enabled else {
            return Err(DomainError::Validation(
                "请给 enabled 或完整题目".into(),
            ));
        };
        sqlx::query(
            "UPDATE reg_quiz_questions SET enabled = $1 WHERE id = $2",
        )
        .bind(en)
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    };
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state
        .repo
        .audit(Some(auth.id), "reg_quiz.update", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "updated": body.id })))
}

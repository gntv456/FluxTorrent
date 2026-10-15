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
use crate::repo::reg_quiz as quiz_repo;
use crate::state::AppState;

use super::guard::staff;
use super::reg_quiz_bank::{validate_quiz, QuizReq};

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
    let auth = staff(&req, &state).await?;
    // P-338（批一百五十四）：本读口此前只过粗闸 staff()，而同文件的写口与
    // 兄弟端点都要「SETTINGS_MANAGE」（class 99 起）。后果：class 91「发布员」能读
    // 注册题库读口全表，写却被 403——**读比写松**，属权限倒挂。
    // 取证：user_can(uid, "settings.manage") 对 class 91 返回 false，
    // 而 GET 这条仍回 200（实测 91 档拿到 email-domains 200 行、
    // roles-dict 5 行、agentrules 12 条）。
    // 修法：读口与写口对齐同一把权限键；语义上「读配置」与「改配置」同档。
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    let rows = quiz_repo::list_questions(&state.repo, limit)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let days = quiz_repo::read_setting(&state.repo, "cheat_accuracy_days")
        .await
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(90)
        .clamp(1, 3650);
    let (attempts, passed) = quiz_repo::attempt_stats(&state.repo, days)
        .await
        .unwrap_or((0, 0));
    let total: i64 = quiz_repo::count_questions(&state.repo)
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
    let id = quiz_repo::add_question(
        &state.repo,
        quiz_repo::NewQuestion {
            category: cat,
            question,
            options: serde_json::Value::from(opts),
            answer_index: ans,
            explanation: expl,
            enabled: body.enabled.unwrap_or(true),
        },
        auth.id,
    )
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
        quiz_repo::update_full(
            &state.repo,
            quiz_repo::NewQuestion {
                category: cat,
                question,
                options: serde_json::Value::from(opts),
                answer_index: ans,
                explanation: expl,
                enabled: body.enabled.unwrap_or(true),
            },
            body.enabled,
            body.id,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        let Some(en) = body.enabled else {
            return Err(DomainError::Validation(
                "请给 enabled 或完整题目".into(),
            ));
        };
        quiz_repo::update_enabled(&state.repo, en, body.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
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

//! 注册答题题库的校验/导入/删除（0372 M5）。
//! 从 reg_quiz_admin.rs 拆出（300 行门禁）；模式同 agent_rule_check.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::repo::reg_quiz as quiz_repo;
use crate::state::AppState;

use super::guard::staff;

/// 题干长度上限（按字符判，与审计口径一致——字节判会把中文题自拒）
const Q_MAX: usize = 1000;
/// 单批导入上限
const IMPORT_MAX: usize = 500;

#[derive(Deserialize, Clone)]
pub(crate) struct QuizReq {
    #[serde(default)]
    pub(crate) category: String,
    pub(crate) question: String,
    pub(crate) options: Vec<String>,
    pub(crate) answer_index: i32,
    #[serde(default)]
    pub(crate) explanation: Option<String>,
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
}

/// 归一化 + 校验；`Ok((category, question, options, answer, explanation))`。
/// add / update / import 三处共用同一把尺（拒绝而不是静默截断）。
pub(crate) fn validate_quiz(
    r: &QuizReq,
) -> Result<(String, String, Vec<String>, i32, Option<String>), String> {
    let question = r.question.trim().to_string();
    let n = question.chars().count();
    if n < 4 || n > Q_MAX {
        return Err(format!("题干长度 4-{Q_MAX} 字（收到 {n}）"));
    }
    let opts: Vec<String> = r
        .options
        .iter()
        .map(|o| o.trim().to_string())
        .filter(|o| !o.is_empty())
        .collect();
    if opts.len() < 2 || opts.len() > 6 {
        return Err(format!("选项须 2-6 个（收到 {}）", opts.len()));
    }
    if r.answer_index < 0 || r.answer_index as usize >= opts.len() {
        return Err("正确答案下标越界".into());
    }
    let category = {
        let c = r.category.trim().to_lowercase();
        if c.is_empty() {
            "general".to_string()
        } else if c.chars().count() > 32 {
            return Err("分类名不超过 32 字".into());
        } else {
            c
        }
    };
    let explanation = r
        .explanation
        .as_ref()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());
    Ok((category, question, opts, r.answer_index, explanation))
}

#[derive(Deserialize)]
struct QuizDelete {
    #[serde(default)]
    ids: Vec<i64>,
}

/// POST /admin/reg-quiz/delete：删除（批量）。
#[post("/admin/reg-quiz/delete")]
async fn reg_quiz_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<QuizDelete>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("没有可删除的题目".into()));
    }
    let n = quiz_repo::delete_questions(&state.repo, &body.ids)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "reg_quiz.delete", Some(n as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

#[derive(Deserialize)]
struct QuizImport {
    questions: Vec<QuizReq>,
}

/// POST /admin/reg-quiz/import：幂等导入（同题干跳过），逐条点名非法项。
#[post("/admin/reg-quiz/import")]
async fn reg_quiz_import(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<QuizImport>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.questions.is_empty() {
        return Err(DomainError::Validation("没有可导入的题目".into()));
    }
    if body.questions.len() > IMPORT_MAX {
        return Err(DomainError::Validation(format!(
            "单次导入上限 {IMPORT_MAX} 条（收到 {}）",
            body.questions.len()
        )));
    }
    // 校验在 H 层逐条收集（非法项点名）；干净行整批交给 repo 单事务落库
    let mut items: Vec<quiz_repo::NewQuestion> = Vec::new();
    let mut rejected: Vec<serde_json::Value> = Vec::new();
    for (i, q) in body.questions.iter().enumerate() {
        let (cat, question, opts, ans, expl) = match validate_quiz(q) {
            Ok(v) => v,
            Err(w) => {
                rejected.push(serde_json::json!({ "index": i, "reason": w }));
                continue;
            }
        };
        items.push(quiz_repo::NewQuestion {
            category: cat,
            question,
            options: serde_json::Value::from(opts),
            answer_index: ans,
            explanation: expl,
            enabled: q.enabled.unwrap_or(true),
        });
    }
    let added = quiz_repo::import_questions(&state.repo, items, auth.id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "reg_quiz.import", Some(added))
        .await;
    Ok(ok(serde_json::json!({
        "added": added,
        "skipped": body.questions.len() as i64 - added,
        "invalid": rejected.len() as i64,
        "rejected": rejected,
    })))
}

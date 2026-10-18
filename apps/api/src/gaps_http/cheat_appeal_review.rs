//! 作弊申诉复核（staff 侧，0369 M3）：队列 + 结论（四眼 + 反向解除）。
//!
//! 结论三选一（appeals.outcome）：
//!   upheld（维持）      → 申诉驳回（status=rejected），处置保留
//!   mitigated（减轻）   → 受理（status=accepted），处置由 staff 另行调整
//!   overturned（推翻）  → 受理（status=accepted）并由 cheat_appeal_reverse
//!     自动反向解除该案已生效的处置（停权/全封/警告/回收流量），案件
//!     sanction.state=reversed、本案证据标 false_positive（准确率统计）。
//!
//! 四眼：**下达该案处置的决定人不得复核针对该案的申诉**（自己判过一遍）；
//! 推翻需操作者等级严格高于目标（ensure_outranks）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz::perm::{CHEAT_CASES_MODERATE, CHEAT_SANCTIONS};
use crate::authz::require_perm;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::cheat_appeals as appeal_repo;
use crate::repo::cheat_reverse::reverse_sanction;
use crate::state::AppState;

/// GET /admin/cheat-appeals：作弊申诉队列（open 优先，可状态过滤）。
#[get("/admin/cheat-appeals")]
pub async fn cheat_appeal_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<QueueQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_perm(&state, &auth, CHEAT_CASES_MODERATE).await?;
    let status = q.status.as_deref().unwrap_or("");
    let rows = appeal_repo::appeal_queue(&state.repo, status)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct QueueQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct ReviewReq {
    /// upheld（维持）/ mitigated（减轻）/ overturned（推翻）
    outcome: String,
    note: String,
}

/// POST /admin/cheat-appeals/{id}/resolve：复核结论 + 反向解除。
#[post("/admin/cheat-appeals/{id}/resolve")]
pub async fn cheat_appeal_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ReviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_perm(&state, &auth, CHEAT_SANCTIONS).await?;
    if !["upheld", "mitigated", "overturned"].contains(&body.outcome.as_str()) {
        return Err(DomainError::Validation(
            "outcome 只认 upheld/mitigated/overturned".into(),
        ));
    }
    let note = body.note.trim();
    if note.is_empty() || note.chars().count() > 4000 {
        return Err(DomainError::Validation("复核说明 1-4000 字符".into()));
    }
    let id = path.into_inner();
    let row = appeal_repo::resolve_target(&state.repo, id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, case_id, sanction)) = row else {
        return Err(DomainError::Validation("申诉不存在或已处理".into()));
    };
    // 四眼：下达该案处置的决定人不得复核该案申诉（approver 已参与第二道
    // 判断，小站点可能因此无人可复核——按文档只锁决定人）
    let decider = sanction
        .as_ref()
        .and_then(|s| s.get("decider"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if decider != 0 && decider == auth.id {
        return Err(DomainError::Validation(
            "决定人不能复核针对自己处置的申诉（四眼原则）".into(),
        ));
    }
    let overturned = body.outcome == "overturned";
    if overturned {
        crate::admin_http::guard::ensure_outranks(
            &state.repo.db,
            auth.class_id,
            uid,
        )
        .await?;
    }
    let status = if overturned || body.outcome == "mitigated" {
        "accepted"
    } else {
        "rejected"
    };

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    appeal_repo::resolve_appeal(
        &mut tx,
        id,
        status,
        &body.outcome,
        auth.id,
        note,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut reversed: Vec<String> = Vec::new();
    if overturned {
        reversed = reverse_sanction(
            &mut tx,
            uid,
            sanction.as_ref(),
            case_id,
            id,
            auth.id,
            note,
        )
        .await?;
    } else if let Some(cid) = case_id {
        appeal_repo::insert_case_note(
            &mut tx,
            cid,
            auth.id,
            format!(
                "申诉 #{id} 复核结论：{}。说明：{note}",
                if body.outcome == "mitigated" {
                    "减轻"
                } else {
                    "维持"
                }
            ),
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    state
        .repo
        .audit(Some(auth.id), "cheat_appeal.resolve", Some(id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        uid,
        Some(auth.id),
        &format!("作弊申诉 #{id} 复核：{}", body.outcome),
    )
    .await;
    notify_appellant(&state, uid, id, &body.outcome, note, &reversed).await;
    Ok(ok(serde_json::json!({
        "id": id,
        "outcome": body.outcome,
        "status": status,
        "reversed": reversed,
    })))
}

/// 通知当事人复核结果（含反向解除明细）。
async fn notify_appellant(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    appeal_id: i64,
    outcome: &str,
    note: &str,
    reversed: &[String],
) {
    let target = appeal_repo::username_email_of(&state.repo, uid).await;
    let Some((username, email)) = target else {
        return;
    };
    let site: String = appeal_repo::site_name(&state.repo).await;
    let verdict = match outcome {
        "overturned" => "推翻（处置已解除）",
        "mitigated" => "减轻",
        _ => "维持原处置",
    };
    crate::mailer::notify(
        &state.repo.db,
        uid,
        email,
        &format!("[{site}] 你的作弊申诉复核结果"),
        &format!(
            "你好 {username}，申诉 #{appeal_id} 的复核结论：{verdict}。\n\
             说明：{note}\n{}",
            if reversed.is_empty() {
                String::new()
            } else {
                format!("已执行：{}。", reversed.join("；"))
            }
        ),
    )
    .await;
}

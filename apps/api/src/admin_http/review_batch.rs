//! 审核台批量裁决（0288，实测审计 P2-1）。
//!
//! 背景：`/admin/torrents/batch` 的九个动作里没有 approve/reject（实测 `action=approve`
//! 回「未知批量动作」），而 `/admin/reviews/decide` 一次只收一条 —— 日均上百待审时
//! 审核员只能逐条点。同时队列原本硬顶 200 条升序，积压后连看见都看不见（见 review.rs）。
//!
//! 口径与单条裁决保持一致（`review_decide`）：
//!   · 只处理**当前仍待审**（approval_status = 0）的种子，逐条判定，不整批回滚；
//!   · 免审连击 / 被拒计数同样累加（`users.approve_streak` / `deny_count`）；
//!   · 自审拦截：操作者本人的种子一律跳过（利益冲突，0285 定的口径）；
//!   · 拒绝必须给字典拒因或自由文本；
//!   · 每条都写 `torrent_operation_logs` + `audit_log`，并发通知。
//! 返回 `{handled, skipped}`，skipped 附原因，审核员看得见「为什么这条没动」。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::services::review_decide::{decide_one, DecideOutcome};
use crate::state::AppState;

use super::guard::staff;

/// 单次批量裁决的种子上限（与批量工作台同量级，防一次打满连接池）
const BATCH_MAX: usize = 100;

#[derive(Deserialize)]
struct BatchReviewReq {
    #[serde(default)]
    torrent_ids: Vec<i64>,
    approve: bool,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    deny_reason_id: Option<i64>,
}

#[derive(Serialize)]
struct Skipped {
    id: i64,
    why: String,
}

#[post("/admin/reviews/batch")]
async fn review_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BatchReviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let ids: Vec<i64> = {
        let mut seen = std::collections::HashSet::new();
        body.torrent_ids
            .iter()
            .filter(|id| seen.insert(**id))
            .copied()
            .collect()
    };
    if ids.is_empty() {
        return Err(DomainError::Validation("未选择种子".into()));
    }
    if ids.len() > BATCH_MAX {
        return Err(DomainError::Validation("单次最多处理 100 条".into()));
    }
    let reason = body.reason.trim().to_string();
    if !body.approve && body.deny_reason_id.is_none() && reason.is_empty() {
        return Err(DomainError::Validation(
            "拒绝必须选择原因或填写理由".into(),
        ));
    }
    if body.approve && body.deny_reason_id.is_some() {
        return Err(DomainError::Validation("通过时不需要拒绝原因".into()));
    }
    // 字典拒因必须存在且启用（一次查完，避免逐条打库）
    if let Some(dr) = body.deny_reason_id {
        if body.approve {
            return Err(DomainError::Validation("通过时不需要拒绝原因".into()));
        }
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM torrent_deny_reasons WHERE id = $1 AND enabled",
        )
        .bind(dr)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation(
                "拒绝原因不存在或已停用".into(),
            ));
        }
    }
    // 逐条裁决：副作用收在服务层用例函数（F2 抽离，SQL 原样搬移），
    // 本层只把用例结果映射成 skipped 条目
    let mut handled: i64 = 0;
    let mut skipped: Vec<Skipped> = Vec::new();
    for id in &ids {
        let why = match decide_one(
            &state,
            auth.id,
            *id,
            body.approve,
            &reason,
            body.deny_reason_id,
            true,
        )
        .await?
        {
            DecideOutcome::NotFound => "种子不存在",
            DecideOutcome::SelfConflict => "不能审核自己发布的种子",
            DecideOutcome::NotPending => "不在待审状态",
            DecideOutcome::Done => {
                handled += 1;
                continue;
            }
        };
        skipped.push(Skipped {
            id: *id,
            why: why.into(),
        });
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.approve {
                "review.approve.batch"
            } else {
                "review.reject.batch"
            },
            None,
        )
        .await;
    crate::torrent_http::bump_list_cache_gen(&state).await;
    // 批量翻转审核态同样要立即刷新 tracker 白名单（P2-2，与单条裁决同口径）
    if handled > 0 {
        crate::http::bump_guard_ver(&state).await;
    }
    Ok(ok(serde_json::json!({
        "handled": handled,
        "skipped": skipped,
        "requested": ids.len(),
    })))
}

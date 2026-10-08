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
    let db = &state.repo.db;
    let mut handled: i64 = 0;
    let mut skipped: Vec<Skipped> = Vec::new();
    for id in &ids {
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
                .bind(id)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(owner_id) = owner else {
            skipped.push(Skipped {
                id: *id,
                why: "种子不存在".into(),
            });
            continue;
        };
        if owner_id == auth.id {
            skipped.push(Skipped {
                id: *id,
                why: "不能审核自己发布的种子".into(),
            });
            continue;
        }
        let n = sqlx::query(
            "UPDATE torrents SET approval_status = $2, \
             approved_at = CASE WHEN $2::smallint = 1 THEN now() ELSE NULL END, \
             deny_reason_id = $3, deny_note = $4 \
             WHERE id = $1 AND approval_status = 0",
        )
        .bind(id)
        .bind(if body.approve { 1 } else { 2 })
        .bind(if body.approve {
            None
        } else {
            body.deny_reason_id
        })
        .bind(if body.approve {
            None
        } else {
            Some(reason.clone())
        })
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n == 0 {
            skipped.push(Skipped {
                id: *id,
                why: "不在待审状态".into(),
            });
            continue;
        }
        // 与 review_decide 同口径的连击/被拒计数（过审清零 deny_count，P2-1）
        let _ = sqlx::query(
            "UPDATE users u SET approve_streak = CASE WHEN $2 THEN \
             u.approve_streak + 1 ELSE 0 END, \
             deny_count = CASE WHEN $2 THEN 0 ELSE u.deny_count + 1 END \
             FROM torrents t \
             WHERE t.id = $1 AND u.id = t.owner_id",
        )
        .bind(id)
        .bind(body.approve)
        .execute(db)
        .await;
        // 过审自动促销 / 组订阅推送：与 review_decide 共用同一实现
        if body.approve {
            super::review_side_effects::apply_approval_side_effects(
                db, *id, auth.id,
            )
            .await;
        }
        let _ = sqlx::query(
            r#"INSERT INTO torrent_operation_logs
            (torrent_id, operator_id, action, detail)
            VALUES ($1, $2, $3, $4::jsonb)"#,
        )
        .bind(id)
        .bind(auth.id)
        .bind(if body.approve { "approve" } else { "reject" })
        .bind(
            serde_json::json!({
                "reason": reason,
                "deny_reason_id": body.deny_reason_id,
                "batch": true,
            })
            .to_string(),
        )
        .execute(db)
        .await;
        super::review_notify::notify_review_result(
            &state,
            *id,
            body.approve,
            &reason,
        )
        .await;
        handled += 1;
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

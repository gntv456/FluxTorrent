//! 自动审种·裁决出口（0400 批 2，从 review_auto.rs 按域拆出，300 行门禁）：
//! worker 引擎 tick 内调的 `/admin/reviews/auto-decide`，副作用走
//! `services::review_decide::decide_one`（与人工单条裁决同一函数）。
//! 规则健康度 sweep 与引擎配置读取留在 review_auto.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;

use crate::errors::{DomainError, DomainResult};
use crate::services::review_decide::{decide_one, DecideOutcome};
use crate::state::AppState;

use super::guard::staff;
use super::review_auto::staff_notify;

#[derive(Deserialize)]
pub(super) struct EngineDecideReq {
    torrent_id: i64,
    approve: bool,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    deny_reason_id: Option<i64>,
}

#[post("/admin/reviews/auto-decide")]
async fn engine_decide(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EngineDecideReq>,
) -> DomainResult<HttpResponse> {
    // 用途头是硬闸：普通客户端即使拿到有效 staff JWT 也走不到这里
    // （这是内部端点，不进 OpenAPI/前端路由清单）
    if req
        .headers()
        .get("x-flux-engine")
        .and_then(|v| v.to_str().ok())
        != Some("auto-review")
    {
        return Err(DomainError::Forbidden);
    }
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    if !body.approve {
        if body.reason.trim().is_empty() && body.deny_reason_id.is_none() {
            return Err(DomainError::Validation(
                "拒绝必须选择原因或填写理由".into(),
            ));
        }
    }
    match decide_one(
        &state,
        auth.id,
        body.torrent_id,
        body.approve,
        &body.reason,
        body.deny_reason_id,
        false,
        true,
    )
    .await?
    {
        // 引擎的操作者不是种子作者（系统审核员），SelfConflict 不该出现；
        // 出现即配置错乱（把某个真人当系统审核员用了），按错误上报
        DecideOutcome::SelfConflict => {
            return Err(DomainError::Internal(anyhow::anyhow!(
                "引擎操作者与发布者相同：系统审核员配置异常"
            )));
        }
        // NotPending 属正常竞态（§13.1：人审先到）：返回 200 + skipped，
        // worker 据此落「人机相遇」对照行，不算失败
        DecideOutcome::NotFound | DecideOutcome::NotPending => {
            return Ok(ok(serde_json::json!({
                "torrent_id": body.torrent_id, "skipped": true,
            })));
        }
        DecideOutcome::Done => {}
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.approve {
                "review.auto_approve"
            } else {
                "review.auto_deny"
            },
            Some(body.torrent_id),
        )
        .await;
    crate::http::bump_guard_ver(&state).await;
    tracing::info!(
        torrent = body.torrent_id,
        approve = body.approve,
        "auto_review: 引擎裁决已落库（decide 同尺）"
    );
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id,
        "approved": body.approve,
        "by": "engine",
    })))
}

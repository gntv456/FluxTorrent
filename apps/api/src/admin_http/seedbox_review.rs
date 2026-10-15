//! seedbox 申报审核（员工侧，0380）。
//!
//! 审核员在这里决定「谁拿得到反作弊放宽」。这是整套设计里**最需要
//! 人工背书**的一环——系统 ASN 自动识别（detected）只给 speed_relax，
//! H&R 豁免与保种倍率必须在这里显式勾选并留名。
//!
//! 【为什么审核权不下放给自动识别】放宽反作弊等于给真实作弊者开门。
//! ASN 抽样天然误判（企业出口、家宽 CDN 都可能命中机房段），一旦
//! 自动识别能直接给 H&R 豁免，误判成本就变成「白嫖做种不计数」。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::guard::staff;
use crate::authz::perm;
use crate::dto::ok;
use crate::domain::seedbox as sb;
use crate::errors::{DomainError, DomainResult};
use crate::repo::seedbox as repo_sb;
use crate::state::AppState;

#[derive(Deserialize)]
struct QueueQ {
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    50
}

#[derive(Deserialize)]
struct ReviewReq {
    claim_id: i64,
    /// 通过还是驳回
    approve: bool,
    /// 审核意见（驳回时必填——让用户知道怎么改）
    #[serde(default)]
    note: String,
    /// 是否给 H&R 豁免（仅 approve 时有意义）
    #[serde(default)]
    hnr_exempt: bool,
    /// 保种倍率加成（默认 1.0；小于 1 会被 domain 层钳到 1.0）
    #[serde(default = "one")]
    bonus_factor: f64,
    /// 是否放宽速度规则
    #[serde(default = "yes")]
    speed_relax: bool,
}
fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}

/// 待审队列（FR-SBOX-003 审核）。
#[get("/admin/seedbox/claims")]
pub(crate) async fn seedbox_review_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<QueueQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::USER_STATUS).await?;
    let limit = q.limit.clamp(1, 200);
    let claims = repo_sb::pending_queue(&state.repo, limit).await?;
    Ok(ok(serde_json::json!({ "claims": claims })))
}

/// 审核落定（FR-SBOX-003）。
///
/// 校验与钳位都在 `domain::seedbox`：
/// · 驳回必须写理由（否则用户不知道怎么改，等于白驳回）；
/// · 三个标志的语义由 `Flags::from_profile` 统一收口——这里传入的
///   `speed_relax` 对 approve 生效；hnr_exempt / bonus_factor 只在
///   approve 时才被采纳（驳回时整套清零）。
#[post("/admin/seedbox/review")]
pub(crate) async fn seedbox_review(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::USER_STATUS).await?;
    if !body.approve && body.note.trim().is_empty() {
        return Err(DomainError::Validation(
            "驳回必须写明理由（用户据此修改后重新申报）".into(),
        ));
    }
    // 倍率下限由 domain 层兜底；此处只拦明显荒唐的上界
    if !(0.0..=10.0).contains(&body.bonus_factor) {
        return Err(DomainError::Validation(
            "保种倍率须在 0 ~ 10 之间".into(),
        ));
    }
    let uid = repo_sb::review(
        &state.repo,
        body.claim_id,
        auth.id,
        body.approve,
        body.note.trim(),
        body.hnr_exempt,
        body.bonus_factor,
        body.speed_relax,
    )
    .await?;
    let Some(uid) = uid else {
        return Err(DomainError::LedgerConflict);
    };
    state
        .repo
        .audit(
            Some(auth.id),
            if body.approve {
                "seedbox.review_approve"
            } else {
                "seedbox.review_reject"
            },
            Some(uid),
        )
        .await;
    // 回读生效标志，让审核员立刻看到「用户实际会拿到什么」
    let f = repo_sb::flags_of(&state.repo, uid).await?;
    Ok(ok(serde_json::json!({
        "user_id": uid,
        "approved": body.approve,
        "effective": {
            "is_seedbox": f.is_seedbox,
            "hnr_exempt": f.hnr_exempt,
            "bonus_factor": f.bonus_factor,
            "speed_relax": f.speed_relax,
        },
    })))
}

#[derive(Deserialize)]
struct RevokeReq {
    user_id: i64,
}

/// 撤回盒子身份（申诉成立 / 误判纠正）。
///
/// 只清画像不动申报历史——历史是可追溯的证据，删了就查不出
/// 「他是不是被放过很多次」。
#[post("/admin/seedbox/revoke")]
pub(crate) async fn seedbox_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RevokeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::USER_STATUS).await?;
    repo_sb::revoke(&state.repo, body.user_id).await?;
    state
        .repo
        .audit(Some(auth.id), "seedbox.revoke", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({ "revoked": body.user_id })))
}

/// 复用 domain 层的倍率下限校验（供单测与将来的批量审核面）。
#[allow(dead_code)]
fn clamped_bonus(v: f64) -> f64 {
    let _ = sb::Flags::from_profile(true, false, false, v, false);
    v.max(1.0)
}

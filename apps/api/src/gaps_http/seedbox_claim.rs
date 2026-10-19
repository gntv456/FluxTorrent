//! seedbox 盒子申报（用户侧，0380）。
//!
//! 三件事：提交申报、看自己的申报历史、看当前生效的标志位。
//! 审核面在 `admin_http/seedbox_review.rs`；判定在 `domain::seedbox`。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::domain::seedbox as sb;
use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::seedbox as repo_sb;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
struct ClaimReq {
    /// 申报的 CIDR 段（最多 20 段，前缀不得过宽）
    cidrs: Vec<String>,
}

/// 提交盒子申报（FR-SBOX-002）。
///
/// 口径：
/// · CIDR 归一化与合法性由 `domain::seedbox::normalize_claims` 判定，
///   前缀过宽（v4 < /24）直接 422——那等于把大半个互联网纳入豁免；
/// · 提交即 pending，**不自动生效**——放宽反作弊必须人工审核背书；
/// · 可重复提交（保留历史供审计），但 pending 时提示避免刷单。
#[utoipa::path(
    post,
    path = "/api/v1/me/seedbox/claim",
    tag = "社区/站务",
    description = "提交 seedbox 盒子申报（需人工审核后生效）。",
    request_body = ClaimReq,
    responses(
        (status = 200, description = "已提交，等待审核"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "CIDR 非法（code 1002）")))]
#[post("/me/seedbox/claim")]
pub(crate) async fn seedbox_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let cidrs = sb::normalize_claims(&body.cidrs)?;
    // 本文件口径承诺「pending 时提示避免刷单」——审计修复（P-388）：这句
    // 提示此前**根本不存在**，提交口无条件插行、也无按人限流，一个人可以把
    // 后台待审队列无限灌。这里先把该人已压着的条数数出来随响应回显。
    // 是否要硬上限（拒绝第 N+1 条）属站点审核口径，未拍板，我不擅自加闸。
    let waiting = repo_sb::count_pending(&state.repo, auth.id).await?;
    let id = repo_sb::submit(&state.repo, auth.id, &cidrs).await?;
    state
        .repo
        .audit(Some(auth.id), "seedbox.claim_submit", Some(id))
        .await;
    let note = if waiting > 0 {
        format!(
            "已提交，等待管理员审核。你已有 {} 条待审申报，重复提交不会更快通过。",
            waiting
        )
    } else {
        "已提交，等待管理员审核。审核通过前不产生任何放宽效果。".to_string()
    };
    Ok(ok(serde_json::json!({
        "claim_id": id,
        "cidrs": cidrs,
        "status": "pending",
        "pending_before": waiting,
        "note": note,
    })))
}

/// 我的申报历史（FR-SBOX-010 隐私口径：只看自己的）。
#[utoipa::path(
    get,
    path = "/api/v1/me/seedbox/claims",
    tag = "社区/站务",
    description = "我的 seedbox 申报历史与审核状态。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/seedbox/claims")]
pub(crate) async fn seedbox_my_claims(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let claims = repo_sb::my_claims(&state.repo, auth.id).await?;
    Ok(ok(serde_json::json!({ "claims": claims })))
}

/// 我当前的盒子标志位（FR-SBOX-004：前端据此显示「已认证盒子」角标）。
///
/// 只回**自己**的标志；不泄露 detected_asns（那是运维信息，且可能
/// 暴露站点识别口径）。
#[utoipa::path(
    get,
    path = "/api/v1/me/seedbox/status",
    tag = "社区/站务",
    description = "我的 seedbox 生效标志位（H&R 豁免 / 保种倍率 / 速度放宽）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/seedbox/status")]
pub(crate) async fn seedbox_my_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let f = repo_sb::flags_of(&state.repo, auth.id).await?;
    Ok(ok(serde_json::json!({
        "is_seedbox": f.is_seedbox,
        "user_declared": f.user_declared,
        "hnr_exempt": f.hnr_exempt,
        "bonus_factor": f.bonus_factor,
        "speed_relax": f.speed_relax,
    })))
}

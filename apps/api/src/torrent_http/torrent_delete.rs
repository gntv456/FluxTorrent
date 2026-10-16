//! 种子删除（M02 管理通道）：软删 + 站方删除表单的理由与附带处置。
//!
//! 从 manage.rs 外移（300 行 / layer 棘轮），一次补齐三件旧缺口：
//!  * 删除不收理由 ⇒ 现在五选机器键 + 白名单，违规/其它必须留说明；
//!  * 删除不动经济 ⇒ 「回收发种人从该种入袋的火花」可选单倍/双倍；
//!  * 删除不权限 ⇒ 「封禁发种」写 `user_permissions` 覆盖，可逆。
//!
//! 兼容：无请求体＝原口径（只软删、理由留空），旧调用方零改动。
//! 处置失败不回滚删除，但一律收进响应 `sanctions.errors` 并在界面点名。

use actix_web::{delete, get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents::{self, delete_dispose as dd};

/// 删除理由白名单（机器键，中文标签在前端字典）。刻意不与举报侧
/// `trump::REASONS` 共用：举报是用户视角的「这条不行」，删除是站方
/// 裁决，「断种」「违规」两项举报侧没有。
const REASONS: [&str; 5] =
    ["dead", "duplicate", "low_quality", "violation", "other"];
/// 必须留书面说明的理由——只有这两项语义不足以复盘裁决
const NOTE_REQUIRED: [&str; 2] = ["violation", "other"];

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteReq {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    note: Option<String>,
    /// 回收发种人从该种入袋的火花
    #[serde(default)]
    deduct_spark: bool,
    /// 双倍扣除是「扣除」的修饰项，未勾扣除时本项不单独成立
    #[serde(default)]
    double_spark: bool,
    /// 封禁发种权限（用户级覆盖，可在权限面板清除回归角色判定）
    #[serde(default)]
    ban_upload: bool,
}

/// 管理通道判定（0159 阈值化）：class ≥ `torrent_delete_class` 才能动他人
async fn staff_class(state: &AppState, class: i16) -> bool {
    let th = torrents::manage_perm::delete_threshold(&state.repo.db).await;
    class >= th
}

/// 取请求体：空＝旧口径（只软删），非空必须解析成 DeleteReq。
///
/// 刻意不用 `Option<web::Json<T>>`：actix 的 Option 提取器把**任何**内层错误
/// 都折成 `Ok(None)`（`actix-web-4.15.0/src/extract.rs` 的 FromRequestOptFuture），
/// 于是写坏的载荷（未知键/类型不符/JSON 不完整）不会报错，而是静默降级成
/// 「一次没有处置的删除」——操作员勾了扣火花、库里什么都没动。
fn parse_body(raw: &[u8]) -> DomainResult<dd::Disposal> {
    if raw.is_empty() {
        return Ok(dd::Disposal::plain());
    }
    let v: DeleteReq = serde_json::from_slice(raw).map_err(|e| {
        tracing::warn!(error = %e, "删除种子请求体解析失败");
        DomainError::Validation("删除请求体格式不正确".into())
    })?;
    to_disposal(&v)
}

/// 形状校验（全在进数据库之前）：坏理由/缺说明/超长说明都拒
fn to_disposal(b: &DeleteReq) -> DomainResult<dd::Disposal> {
    let reason = b
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty());
    if let Some(r) = reason {
        if !REASONS.contains(&r) {
            return Err(DomainError::Validation("删除理由不合法".into()));
        }
    }
    let note = b.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if note.map_or(false, |n| n.chars().count() > 500) {
        return Err(DomainError::Validation("说明需 ≤500 字".into()));
    }
    if reason.map_or(false, |r| NOTE_REQUIRED.contains(&r)) && note.is_none() {
        return Err(DomainError::Validation(
            "违规与其它理由必须填写说明".into(),
        ));
    }
    let disposing = b.deduct_spark || b.ban_upload;
    if disposing && reason.is_none() {
        return Err(DomainError::Validation(
            "执行处置必须先选择删除理由".into(),
        ));
    }
    Ok(dd::Disposal {
        reason: reason.map(str::to_owned),
        note: note.map(str::to_owned),
        deduct_spark: b.deduct_spark,
        double_spark: b.double_spark,
        ban_upload: b.ban_upload,
    })
}

/// 删除表单取数（弹窗打开时拉一次）：同伴计数 + 可回收火花 + 能否处置。
#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/delete-preview",
    tag = "种子",
    description = "删除弹窗取数：同伴计数、发种人入袋火花、可否附加处置。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[get("/torrents/{id}/delete-preview")]
pub(crate) async fn delete_preview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let v = torrents::delete_view(&state.repo.db, id).await?;
    // 能删才给看：作者看自己未过审的种（与软删口同判据）
    let staff = staff_class(&state, auth.class_id as i16).await;
    let is_owner = v.owner_id == Some(auth.id);
    if !staff && !(is_owner && v.approval_status != 1) {
        return Err(DomainError::Forbidden);
    }
    let mut j = v.json();
    // 处置只对「管理人员 × 他人种子（且有发种人）」成立，作者自删不配这套勾。
    // 判据与 delete_torrent 里那句必须逐字一致，否则弹窗给了勾、提交被拒
    j["can_dispose"] =
        serde_json::json!(staff && !is_owner && v.owner_id.is_some());
    Ok(ok(j))
}

/// 删除种子（软删 approval_status=3；staff 任意删，作者仅限未过审）。
/// 带请求体时同时执行站方处置并写台账，见模块头。
#[utoipa::path(
    delete,
    path = "/api/v1/torrents/{id}",
    tag = "种子",
    description = "软删种子（进回收站，管理员口可硬删）；可携带理由与处置。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    request_body = DeleteReq,
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[delete("/torrents/{id}")]
pub(crate) async fn delete_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Bytes,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let d = parse_body(&body)?;
    let disposing = d.deduct_spark || d.ban_upload;
    let mut owner = None;
    if disposing {
        // 资格判定必须在软删之前：删除本身允许作者删自己未过审的种，
        // 但扣火花/禁发种是站方对**他人**的裁决，对自己不成立
        let v = torrents::delete_view(&state.repo.db, id).await?;
        let staff = staff_class(&state, auth.class_id as i16).await;
        if !staff || v.owner_id == Some(auth.id) || v.owner_id.is_none() {
            return Err(DomainError::Validation(
                "追加处置仅管理人员对他人种子可用".into(),
            ));
        }
        owner = v.owner_id;
    }
    // 作者通道只能删自己未过审的种，管理通道不限（域内判定）
    let actor = (auth.id, auth.class_id as i16);
    torrents::delete_torrent(&state.repo.db, id, actor).await?;
    let mut outcome = dd::DisposalOutcome::default();
    if disposing {
        outcome =
            dd::apply_disposal(&state.repo.db, id, auth.id, owner, &d).await;
    }
    if let Err(e) =
        dd::write_delete_log(&state.repo.db, id, auth.id, &d, &outcome).await
    {
        outcome.errors.push(e);
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.delete", Some(id))
        .await;
    // 软删把审核态翻成 3，tracker 白名单（只载 0/1）须立即刷新，
    // 否则一个 guard 周期内被删种仍可 announce（P2-2）
    crate::http::bump_guard_ver(&state).await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(dd::disposal_json(id, &outcome)))
}

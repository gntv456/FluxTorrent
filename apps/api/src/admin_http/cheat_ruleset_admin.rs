//! 反作弊规则集版本管理面（0391）。
//!
//! 三屏：版本列表（含参数快照与生效状态）/ 影子对比 / 回测。
//!
//! 【这块存在的理由】反作弊权重一改就可能给一批人开案。没有
//! 「先存成草稿 → 影子跑一轮看会多判多少人 → 回测历史 → 才切换」
//! 这条路，改参数就只能闭眼按扳机。这里把那条路做成界面。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz::perm;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::cheat_ruleset as repo_cr;
use crate::state::AppState;

/// 版本列表（新到旧）。
#[get("/admin/cheat/rulesets")]
pub(crate) async fn list_rulesets(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    Ok(ok(serde_json::json!({
        "rulesets": repo_cr::list_rulesets(&state.repo).await?,
        "shadow_mode": repo_cr::shadow_mode(&state.repo).await?,
    })))
}

#[derive(Deserialize)]
struct CreateReq {
    /// 以当前 site_settings 的 risk_* 为基线，可选覆盖若干键。
    #[serde(default)]
    overrides: std::collections::BTreeMap<String, String>,
    note: String,
}

/// 存一版草稿（从当前设置取基线 + 覆盖）。
///
/// 刻意**不**直接改 site_settings：草稿期间生效参数不动，
/// 否则「影子对比」比的就是同一个东西。
#[post("/admin/cheat/rulesets")]
pub(crate) async fn create_ruleset(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    if body.overrides.is_empty() {
        return Err(DomainError::Validation(
            "至少覆盖一个参数，否则这版与当前版本无差别".into(),
        ));
    }
    let v =
        repo_cr::create_ruleset(&state.repo, &body.overrides, body.note.trim())
            .await?;
    state
        .repo
        .audit(Some(auth.id), "cheat.ruleset_create", None)
        .await;
    Ok(ok(serde_json::json!({ "version": v })))
}

#[derive(Deserialize)]
struct ActivateReq {
    version: i32,
}

/// 切换生效版本。
///
/// **副作用要说清**：切换后下一轮 risk_score 就按新权重算分。
/// 影子模式开着时只算不开案；关着就直接开案——所以这个端点
/// 在响应里回显当前影子模式状态，避免运营误以为「存了就生效」。
#[post("/admin/cheat/rulesets/activate")]
pub(crate) async fn activate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ActivateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    let n = repo_cr::activate(&state.repo, body.version).await?;
    if n == 0 {
        return Err(DomainError::NotFound(body.version as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "cheat.ruleset_activate", None)
        .await;
    Ok(ok(serde_json::json!({
        "activated": body.version,
        "shadow_mode": repo_cr::shadow_mode(&state.repo).await?,
        "note": if repo_cr::shadow_mode(&state.repo).await? {
            "影子模式开：只算分不开案，先看对比面"
        } else {
            "影子模式关：下一轮即按新权重开案"
        },
    })))
}

#[derive(Deserialize)]
struct ShadowReq {
    /// 必填。旧写法 `#[serde(default)]`：`POST /admin/cheat/shadow` 少带
    /// `on` 就等于**关掉影子模式**——而影子模式是「只算分不开案」的保护，
    /// 关掉后下一轮 `risk_score` 会直接对真人开案件（现网读侧缺省是 '1'
    /// 开着，见 worker `risk_score.rs:62` 的 COALESCE）。
    /// 与 `PUT /admin/classes/{id}` 的 demotable 同一族：缺省语义会扩大
    /// 破坏范围的字段，一律要求显式给出。
    on: Option<bool>,
}

/// 影子模式开关。
#[post("/admin/cheat/shadow")]
pub(crate) async fn set_shadow(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShadowReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    let on = body.on.ok_or_else(|| {
        DomainError::Validation(
            "必须显式给出 on：省略会被当成关掉影子模式（只算分不开案的保护）"
                .into(),
        )
    })?;
    repo_cr::set_shadow_mode(&state.repo, on).await?;
    state
        .repo
        .audit(Some(auth.id), "cheat.shadow_set", None)
        .await;
    Ok(ok(serde_json::json!({ "shadow_mode": on })))
}

/// 影子对比：谁会因此被开案、谁会掉档。
#[get("/admin/cheat/shadow")]
pub(crate) async fn shadow_diff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    let s = repo_cr::shadow_summary(&state.repo).await?;
    Ok(ok(serde_json::json!({
        "shadow_mode": repo_cr::shadow_mode(&state.repo).await?,
        "would_open": s.would_open,
        "total": s.total,
        "top": repo_cr::shadow_top(&state.repo, 50).await?,
    })))
}

#[derive(Deserialize)]
struct BacktestReq {
    from_ver: i32,
    to_ver: i32,
    #[serde(default = "default_days")]
    window_days: i32,
}
fn default_days() -> i32 {
    90
}

/// 排一次回测（异步落库，返回 run id）。
///
/// 回测要扫窗口内全部证据，比评分重得多，故**不在请求里同步算**——
/// 提交后由面板轮询 run 状态。这里只记录请求，等 worker 跑。
#[post("/admin/cheat/backtest")]
pub(crate) async fn run_backtest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BacktestReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    if body.from_ver <= 0 || body.to_ver <= 0 || body.from_ver == body.to_ver {
        return Err(DomainError::Validation(
            "回测需要两个不同的正整数版本号".into(),
        ));
    }
    let id = repo_cr::enqueue_backtest(
        &state.repo,
        body.from_ver,
        body.to_ver,
        body.window_days.clamp(1, 365),
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "cheat.backtest_run", None)
        .await;
    Ok(ok(serde_json::json!({ "run_id": id })))
}

/// 回测历史 + 单次详情。
#[get("/admin/cheat/backtests")]
pub(crate) async fn backtests(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::CHEAT_CASES_MODERATE)
        .await?;
    Ok(ok(serde_json::json!({
        "runs": repo_cr::list_backtests(&state.repo, 20).await?,
    })))
}

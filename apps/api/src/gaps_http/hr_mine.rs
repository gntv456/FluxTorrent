//! GET /me/hr —— 我的 H&R 列表（分页 + 状态筛选 + KPI 计数）。
//!
//! 从 `hr.rs` 拆出（那个文件因赦免三端点 + 本次新增的分页越过 300 行
//! 门禁）。SQL 住在 `repo/hr.rs`，本文件只做参数归一与 JSON 组装，
//! 因此**不含任何 sqlx 引用**。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::hr as repo_hr;
use crate::state::AppState;

#[derive(Deserialize)]
struct MyHrQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    per: Option<i64>,
    /// open|satisfied|violated|pardoned，其余值按不筛处理
    #[serde(default)]
    status: Option<String>,
}

#[derive(serde::Serialize)]
struct MyHrResp {
    rows: Vec<repo_hr::HrRow>,
    total: i64,
    page: i64,
    per: i64,
    /// 当前生效的状态筛选（回显白名单结果，不透传原始输入）
    status: Option<String>,
    /// 四状态全量计数（不受 status 筛选影响，供 KPI 条用）
    counts: serde_json::Value,
    /// 站点缺省口径：与 worker 建快照同源的 hr_hours 设定，
    /// 供前端动态渲染规则文案（此前写死 120h 与实际执行 48h 矛盾）
    default_seed_hours: i32,
    default_days: i32,
}

#[get("/me/hr")]
pub async fn my_hr_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<MyHrQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let per = q.per.unwrap_or(20).clamp(1, 100);
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let status = repo_hr::status_filter(q.status.as_deref());
    let off = crate::dto::page_offset(page, per);
    let rows = repo_hr::my_rows(&state.repo, auth.id, status, per, off).await?;
    // 空结果集拿不到窗口计数与标量计数，退回全 0（前端据此画
    // 「没有记录」而不是一个空页码）
    let first = rows.first();
    let counts = serde_json::json!({
        "open": first.map(|r| r.n_open).unwrap_or(0),
        "satisfied": first.map(|r| r.n_satisfied).unwrap_or(0),
        "violated": first.map(|r| r.n_violated).unwrap_or(0),
        "pardoned": first.map(|r| r.n_pardoned).unwrap_or(0),
    });
    let total = first.map(|r| r.total).unwrap_or(0);
    Ok(ok(MyHrResp {
        rows,
        total,
        page,
        per,
        status: status.map(str::to_string),
        counts,
        default_seed_hours: repo_hr::hours_setting(&state.repo.db).await,
        default_days: repo_hr::days_setting(&state.repo.db).await,
    }))
}

//! GET /me/spark/ledger —— 火花流水（筛选 + 分页）。
//!
//! 从 `voucher_use.rs` 拆出（那个文件压在 300 行门禁上），SQL 住在
//! `repo/spark_ledger.rs`。本文件只做参数归一与 JSON 组装。
//!
//! 参数一律「认不出的值 = 不筛」，不回 400：流水页的筛选是浏览辅助，
//! 拿一个拼错的 kind 打断阅读没有必要（与 /forums/{id}/topics 的
//! `type` 白名单回落同一口径）。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::spark_ledger as repo_ledger;
use crate::state::AppState;

/// 每页条数：与拆出前一致（50），上界 100 防一次拉空内存。
const PER_DEFAULT: i64 = 50;
const PER_MAX: i64 = 100;

#[derive(Deserialize)]
struct LedgerQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    per: Option<i64>,
    /// 精确到某一类（前端下拉给白名单值）
    #[serde(default)]
    kind: Option<String>,
    /// in=收入 / out=支出 / 其余=全部
    #[serde(default)]
    dir: Option<String>,
    /// RFC3339 起（含）
    #[serde(default)]
    from: Option<String>,
    /// RFC3339 止（不含）
    #[serde(default)]
    to: Option<String>,
}

fn ts(raw: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

#[utoipa::path(
    get,
    path = "/api/v1/me/spark/ledger",
    tag = "经济",
    description = "我的火花流水（可按类型/方向/时间筛，分页）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/spark/ledger")]
pub(crate) async fn my_ledger(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<LedgerQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let per = q.per.unwrap_or(PER_DEFAULT).clamp(1, PER_MAX);
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let kind = q.kind.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let dir = match q.dir.as_deref() {
        Some("in") | Some("out") => q.dir.as_deref(),
        _ => None,
    };
    let f = repo_ledger::Filter {
        kind,
        dir,
        from: ts(q.from.as_deref()),
        to: ts(q.to.as_deref()),
    };
    let rows =
        repo_ledger::ledger(&state.repo, auth.id, &f, per, (page - 1) * per)
            .await?;
    let total = repo_ledger::count_ledger(&state.repo, auth.id, &f).await?;
    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "page": page,
        "per": per,
        "kind": kind,
        "dir": dir,
    })))
}

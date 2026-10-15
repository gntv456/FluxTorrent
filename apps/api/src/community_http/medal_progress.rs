//! 自动成就进度（0382 的读面）。
//!
//! 三件事：
//!   1. 我的成就进度（已达成 / 进行中，带比例与「还差多少」）；
//!   2. 阈值类成就的当前值（口径与 worker 取数同源，避免两处算出不同数）；
//!   3. 口径未接上的成就**显式标出**，而不是给一个永远 0% 的进度条。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::domain::medal as md;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::medal_auto as repo_ma;
use crate::state::AppState;

/// 每个规则的展示单位（前端画进度条要用）。
///
/// 单独列出而不是散在各 handler：文案口径必须唯一一处，
/// 否则「100 GB / 50 GB」这种读起来矛盾的数字迟早出现。
fn unit_of(rule: &str) -> &'static str {
    match rule {
        "uploaded" => "GB",
        "seed_hours" => "小时",
        "uploads" => "个种子",
        "registered_days" => "天",
        _ => "",
    }
}

/// 我的自动成就进度。
///
/// 返回字段说明：
///   * `current`/`target`/`ratio`/`text` —— 进度条四件套；
///   * `awarded` —— 是否已拿到（拿到后 `ratio` 恒 1.0）；
///   * `unwired` —— 口径没接上的成就（`current` 为 null）。
///     这条最要紧：若把未知规则当 0 返回，用户会看到一条永远 0% 的
///     进度条且毫无线索；显式标出才有人会去补规则。
#[utoipa::path(
    get,
    path = "/api/v1/medals/progress",
    tag = "社区/勋章",
    description = "我的自动成就进度（含当前值 / 目标 / 比例 / 还差多少）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/medals/progress")]
pub(crate) async fn my_medal_progress(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = repo_ma::my_progress(&state.repo, auth.id).await?;
    let stats = repo_ma::my_stats(&state.repo, auth.id).await?;

    let mut items = Vec::with_capacity(rows.len());
    let mut unwired: Vec<String> = Vec::new();
    for r in rows {
        let rule = r.auto_rule.as_deref().unwrap_or("");
        // threshold 类的当前值取自实时统计；event 类的取自 medal_progress
        let live = if r.current > 0 {
            Some(r.current)
        } else {
            stats.of(rule)
        };
        match live {
            None if !r.awarded => unwired.push(rule.to_string()),
            _ => {}
        }
        let current = live.unwrap_or(0);
        let target = r.auto_target;
        items.push(serde_json::json!({
            "id": r.id,
            "name": r.name,
            "description": r.description,
            "asset_ref": r.asset_ref,
            "rarity": r.rarity,
            "category_id": r.category_id,
            "rule": rule,
            "unit": unit_of(rule),
            "current": if live.is_none() && !r.awarded {
                serde_json::Value::Null
            } else {
                serde_json::json!(current)
            },
            "target": target,
            "ratio": md::progress_ratio(current, target),
            "text": md::progress_text(current, target, unit_of(rule)),
            "remaining": (target - current).max(0),
            "awarded": r.awarded,
            "awarded_at": r.awarded_at,
            "wearing": r.wearing,
            "progress_visible": r.progress_visible,
            "sort_order": r.sort_order,
        }));
    }
    items.sort_by(|a, b| {
        // 未达成的在前（用户更关心「还差什么」），组内按站长排的顺序。
        let ax = a["awarded"].as_bool().unwrap_or(false);
        let bx = b["awarded"].as_bool().unwrap_or(false);
        ax.cmp(&bx).then_with(|| {
            a["sort_order"].as_i64().cmp(&b["sort_order"].as_i64())
        })
    });
    Ok(ok(serde_json::json!({
        "items": items,
        "stats": {
            "uploaded_gb": stats.uploaded / 1024 / 1024 / 1024,
            "seed_hours": stats.seed_hours,
            "uploads": stats.uploads,
            "registered_days": stats.registered_days,
        },
        "unwired": unwired,
    })))
}

//! 反作弊规则准确率（0372 M5）：按 agent 家族统计误判率与处置结论分布。
//!
//! 数据源全是 M1~M3 已有的台账，不需要新表：
//!   cheat_events.false_positive（申诉 overturned 时由 cheat_appeal_reverse
//!     置位）→ 规则误判；cheat_events.resolved_at → 已处置；
//!   reports.sanction（M3 处置状态机）→ applied/reversed/expired 分布。
//!
//! 用途：站长据此调规则权重/阈值——误判率高的家族要么放宽阈值，要么补
//! 二次确认；长期 applied 无 reversed 的家族说明规则稳。
//!
//! 口径注意：`false_positive` 只在本案证据被推翻时置位（申诉 overturned），
//! 不是「所有被推翻的处置都逐个回填」——所以 accuracy 是**下界估计**，
//! 分母用「已处置证据数」（resolved 且非本案外的孤立行不计）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::repo::cheat_accuracy as acc_repo;
use crate::state::AppState;

use super::guard::staff;

#[derive(serde::Serialize)]
struct FamilyRow {
    family: String,
    events: i64,
    resolved: i64,
    false_positive: i64,
    hits: i64,
    /// 1 - fp/resolved（resolved=0 时为 null，避免「零样本=100% 准」误导）
    accuracy: Option<f64>,
}

#[derive(serde::Deserialize)]
struct AccQuery {
    days: Option<i64>,
}

/// GET /admin/cheat-accuracy：家族级准确率 + 处置结论分布。
#[get("/admin/cheat-accuracy")]
async fn cheat_accuracy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<AccQuery>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-338（批一百五十四）：本读口只过粗闸 staff()，而它把反作弊家族的
    // 命中率/误报数/处置结论分布整表摊给任何 staff —— class 91「发布员」
    // 能读到**误报统计**（等于反作弊模型的内部指标）。
    // 对齐既有权限键 audit.view（class 90 起，与 cheat.cases.moderate 同档）：
    // 「看处置台账」与「看它的准确率」是同一档信息。
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AUDIT_VIEW)
        .await?;
    let days = match q.days {
        Some(d) => d.clamp(1, 3650),
        None => acc_repo::accuracy_days(&state.repo).await.clamp(1, 3650),
    };
    let raw = acc_repo::family_accuracy(&state.repo, days)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let families: Vec<FamilyRow> = raw
        .into_iter()
        .map(|r| {
            let accuracy = if r.resolved > 0 {
                Some(1.0 - (r.false_positive as f64) / (r.resolved as f64))
            } else {
                None
            };
            FamilyRow {
                family: r.family,
                events: r.events,
                resolved: r.resolved,
                false_positive: r.false_positive,
                hits: r.hits,
                accuracy,
            }
        })
        .collect();
    // 处置结论分布（M3 状态机；sanction 为 NULL 的 = 仅结案未处置）
    let sanctions = acc_repo::sanction_distribution(&state.repo, days)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "days": days,
        "families": families,
        "sanctions": sanctions,
    }))))
}

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

#[derive(sqlx::FromRow)]
struct RawFamily {
    family: String,
    events: i64,
    resolved: i64,
    false_positive: i64,
    hits: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct SanctionRow {
    kind: String,
    total: i64,
    applied: i64,
    reversed: i64,
    expired: i64,
    pending: i64,
}

/// GET /admin/cheat-accuracy：家族级准确率 + 处置结论分布。
#[get("/admin/cheat-accuracy")]
async fn cheat_accuracy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<AccQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let days = match q.days {
        Some(d) => d.clamp(1, 3650),
        None => sqlx::query_scalar(
            "SELECT COALESCE((SELECT value::bigint FROM site_settings \
             WHERE name = 'cheat_accuracy_days'), 90)",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(90)
        .clamp(1, 3650),
    };
    // 家族 = agent 的冒号前缀（`ghost:`→ghost；无前缀的 down_under/bitthief
    // 自成一家族）。group by 1 用别名在 PG 里可用（输出列名）。
    let raw: Vec<RawFamily> = sqlx::query_as(
        "SELECT \
           CASE WHEN position(':' in agent) > 0 \
                THEN split_part(agent, ':', 1) ELSE agent END AS family, \
           count(*)::bigint AS events, \
           count(*) FILTER (WHERE resolved_at IS NOT NULL)::bigint \
             AS resolved, \
           count(*) FILTER (WHERE false_positive IS TRUE)::bigint \
             AS false_positive, \
           COALESCE(sum(hits), 0)::bigint AS hits \
         FROM cheat_events \
         WHERE last_seen > now() - ($1::bigint * interval '1 day') \
         GROUP BY 1 ORDER BY false_positive DESC, events DESC",
    )
    .bind(days)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let families: Vec<FamilyRow> = raw
        .into_iter()
        .map(|r| {
            let accuracy = if r.resolved > 0 {
                Some(
                    1.0 - (r.false_positive as f64) / (r.resolved as f64),
                )
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
    let sanctions: Vec<SanctionRow> = sqlx::query_as(
        "SELECT COALESCE(sanction->>'kind', 'none') AS kind, \
           count(*)::bigint AS total, \
           count(*) FILTER (WHERE sanction->>'state' = 'applied')::bigint \
             AS applied, \
           count(*) FILTER (WHERE sanction->>'state' = 'reversed')::bigint \
             AS reversed, \
           count(*) FILTER (WHERE sanction->>'state' = 'expired')::bigint \
             AS expired, \
           count(*) FILTER (WHERE sanction->>'state' = 'pending')::bigint \
             AS pending \
         FROM reports \
         WHERE ref_type = 'cheat' \
           AND created_at > now() - ($1::bigint * interval '1 day') \
         GROUP BY 1 ORDER BY total DESC",
    )
    .bind(days)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "days": days,
        "families": families,
        "sanctions": sanctions,
    }))))
}

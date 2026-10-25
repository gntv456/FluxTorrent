//! 自动审种管理面（0400/0401 批 2）：统计/台账/回放/预设与站型包套用。
//!
//! 四个端点全部只读或幂等写设置组，复用 settings 保存链的校验与失效；
//! 权限沿用 SETTINGS_MANAGE（与设置页一致——这些端点本质是设置页的
//! 「批量带出」按钮后端）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// GET /admin/auto-review/stats：影子四指标 + 今日计数 + 熔断状态。
/// 配置页统计条与摘要页共用（§3.1/§15.2）。
#[get("/admin/auto-review/stats")]
async fn stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let db = &state.repo.db;
    // 机器先出意见、人工后裁决的样本（影子校准的分母口径，§13.3）
    let (agree, machine_pass_human_deny, machine_deny_human_pass, pending) =
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT \
                count(*) FILTER (WHERE \
                    (l.verdict = 0 AND t.approval_status = 1) OR \
                    (l.verdict = 1 AND t.approval_status = 2)), \
                count(*) FILTER (
                  WHERE l.verdict = 0 AND t.approval_status = 2), \
                count(*) FILTER (
                  WHERE l.verdict = 1 AND t.approval_status = 1), \
                count(*) FILTER (WHERE t.approval_status = 0) \
             FROM auto_review_log l JOIN torrents t ON t.id = l.torrent_id \
             WHERE l.decided_by = 0",
        )
        .bind(0)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let total = agree + machine_pass_human_deny + machine_deny_human_pass;
    let agree_pct = if total > 0 { agree * 100 / total } else { 0 };
    // verdict 列是 smallint（int2）：按 i64 解码会报「类型不匹配」
    // ⇒ 只要当天有台账行，这个口就 500（表一有数据就必炸）。
    let today: Vec<(i16, i64)> = sqlx::query_as(
        "SELECT l.verdict, count(*) FROM auto_review_log l \
         WHERE l.decided_by = 0 AND l.created_at >= \
         (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
         AT TIME ZONE 'Asia/Shanghai') \
         GROUP BY l.verdict",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (t_pass, t_deny, t_postpone) = (
        today
            .iter()
            .find(|(v, _)| *v == 0)
            .map(|(_, c)| *c)
            .unwrap_or(0),
        today
            .iter()
            .find(|(v, _)| *v == 1)
            .map(|(_, c)| *c)
            .unwrap_or(0),
        today
            .iter()
            .find(|(v, _)| *v == 2)
            .map(|(_, c)| *c)
            .unwrap_or(0),
    );
    // 影子→执法门槛（§6）
    let (min_samples, min_agree): (i64, i64) = sqlx::query_as(
        "SELECT \
            COALESCE((SELECT value::bigint FROM site_settings \
              WHERE name = 'auto_review_shadow_min_samples'), 200), \
            COALESCE((SELECT value::bigint FROM site_settings \
              WHERE name = 'auto_review_min_agree_pct'), 85)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let health: Vec<(String, i32, i32)> = sqlx::query_as(
        "SELECT rule, overturns, appeal_wins FROM auto_review_rule_health \
         ORDER BY (overturns + 2 * appeal_wins) DESC",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 切执法提示文案（0402 登记的设置键）：站点可编辑，
    // 前端在设置页/台账页展示。原先只落库不读取——后台改得动、
    // 界面回显正常、实际哪都不显示（假开关）。
    let switch_hint: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'auto_review_switch_hint'), '')",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "switch_hint": switch_hint,
        "shadow": {
            "samples": total,
            "agree": agree, "agree_pct": agree_pct,
            "false_deny": machine_deny_human_pass,
            "false_pass": machine_pass_human_deny,
            "min_samples": min_samples, "min_agree_pct": min_agree,
            "ready": total >= min_samples && agree_pct >= min_agree,
        },
        "today": { "pass": t_pass, "deny": t_deny, "postpone": t_postpone },
        "pending_human": pending,
        "rule_health": health,
    })))
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default)]
    rule: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}
fn default_limit() -> i64 {
    50
}

/// GET /admin/auto-review/logs：台账查询（§15.2；rule 筛选支持按规则反查）。
#[get("/admin/auto-review/logs")]
async fn logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LogsQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let db = &state.repo.db;
    // hit_rules 是 JSONB 数组，按 rule 反查用 EXISTS 子查询（不用 @>，
    // 元素是对象不是标量）
    let rows: Vec<(
        i64,
        i64,
        String,
        String,
        Option<i16>,
        i16,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT l.id, l.torrent_id, t.name, l.hit_rules::text, \
                    l.final_score, l.verdict, l.created_at \
             FROM auto_review_log l JOIN torrents t ON t.id = l.torrent_id \
             WHERE l.decided_by = 0 \
               AND ($1::text IS NULL OR EXISTS (SELECT 1 FROM \
                     jsonb_array_elements(l.hit_rules) e \
                     WHERE e->>'rule' = $1)) \
             ORDER BY l.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(&q.rule)
    .bind(q.limit.min(200))
    .bind(q.offset)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, tid, name, hits, score, verdict, at)| {
            serde_json::json!({
                "id": id, "torrent_id": tid, "name": name,
                "hits": serde_json::from_str::<serde_json::Value>(&hits)
                    .unwrap_or(serde_json::json!([])),
                "score": score, "verdict": verdict, "created_at": at,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "items": items,
        "limit": q.limit,
        "offset": q.offset,
    })))
}

//! 运营聚合双看板（0506 运营体检二批，台账 #A4）：
//! ① GET /admin/attendance-stats —— 签到 14 天趋势（签到人数/占比/
//!    平均连签/补签占比）。此前 /admin/attendance 只有分页明细行，
//!    「日签到活跃」「本周注册者首签率」无处可查。
//! ② GET /admin/hr-stats —— H&R 状态分布（open/satisfied/violated/
//!    pardoned 计数 + 近 14 天新增违规趋势 + 豁免率）。此前
//!    /admin/hr/records 同样只有明细；hr_snapshots 快照表数据早已
//!    具备，只缺读口。
//! 两口都是 SETTINGS_VIEW 只读聚合，单查询 GROUP BY，无新表。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// GET /admin/attendance-stats —— 签到 14 天趋势（SETTINGS_VIEW）。
#[get("/admin/attendance-stats")]
pub async fn attendance_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    // 日聚合：签到人数 / 补签数 / 平均连签；基数=当日活跃用户
    // （last_seen 口径），占比=签到人数/活跃数。
    let rows: Vec<(chrono::NaiveDate, i64, i64, f64, i64)> = sqlx::query_as(
        r#"
        SELECT d::date,
               count(a.user_id)::bigint,
               count(a.user_id) FILTER (WHERE a.makeup)::bigint,
               COALESCE(avg(a.streak), 0),
               (SELECT count(*)::bigint FROM users u
                 WHERE COALESCE(u.last_seen_at, u.created_at)::date = d::date
                   AND u.status < 2)
        FROM generate_series(
               (now() - interval '13 days')::date, now()::date,
               interval '1 day') d
        LEFT JOIN attendance a ON a.date = d::date
        GROUP BY d ORDER BY d
        "#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|(date, checkins, makeups, avg_streak, active)| {
            serde_json::json!({
                "date": date.to_string(),
                "checkins": checkins,
                "makeups": makeups,
                "avgStreak": (avg_streak * 10.0).round() / 10.0,
                "activeUsers": active,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "days": out })))
}

/// GET /admin/hr-stats —— H&R 状态分布 + 近 14 天新增违规（SETTINGS_VIEW）。
#[get("/admin/hr-stats")]
pub async fn hr_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    // 状态分布（全量）
    let dist: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*)::bigint FROM hr_snapshots \
         GROUP BY status ORDER BY count(*) DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 近 14 天新增违规趋势（violations 表按日）
    let trend: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        r#"
        SELECT d::date, count(v.id)::bigint
        FROM generate_series(
               (now() - interval '13 days')::date, now()::date,
               interval '1 day') d
        LEFT JOIN hr_violations v
          ON v.detected_at::date = d::date
        GROUP BY d ORDER BY d
        "#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = dist.iter().map(|(_, n)| n).sum();
    let pardoned: i64 = dist
        .iter()
        .filter(|(s, _)| s == "pardoned")
        .map(|(_, n)| n)
        .sum();
    Ok(ok(serde_json::json!({
        "distribution": dist
            .iter()
            .map(|(s, n)| serde_json::json!({"status": s, "count": n}))
            .collect::<Vec<_>>(),
        "total": total,
        "pardonRate": if total > 0 {
            ((pardoned as f64 / total as f64) * 1000.0).round() / 10.0
        } else { 0.0 },
        "trend": trend
            .iter()
            .map(|(d, n)| {
                serde_json::json!({"date": d.to_string(), "violations": n})
            })
            .collect::<Vec<_>>(),
    })))
}

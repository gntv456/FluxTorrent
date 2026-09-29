use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 运营概览：待审/举报/用户/种子计数
#[get("/admin/overview")]
async fn admin_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (pending, reports, users, torrents, banned): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
            (SELECT count(*) FROM torrents WHERE approval_status = 0), \
            (SELECT count(*) FROM reports WHERE status = 0), \
            (SELECT count(*) FROM users), \
            (SELECT count(*) FROM torrents), \
            (SELECT count(*) FROM users WHERE status >= 2)",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "pending_reviews": pending, "open_reports": reports,
        "users": users, "torrents": torrents, "banned_users": banned,
        "operator": auth.id,
        // E16 数据大盘三块：14 天注册/发种趋势（今日倒排）、做种健康度
        // （活种率/死种数）、周活（7 天内有 announce 或登录的用户数）。
        // 全部子查询点查级；空库/新站各值自然为 0，不做特殊分支。
        "trend": trend(&state).await,
        "health": health(&state).await,
        "wau": wau(&state).await,
    })))
}

/// 近 14 天注册/发种趋势：数组 [日期, 注册数, 发种数]，旧→新
async fn trend(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> serde_json::Value {
    let rows: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT d::date, \
            (SELECT count(*) FROM users u \
             WHERE u.created_at::date = d::date), \
            (SELECT count(*) FROM torrents t \
             WHERE t.created_at::date = d::date) \
         FROM generate_series(current_date - 13, current_date, \
         interval '1 day') d",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    serde_json::json!(rows
        .into_iter()
        .map(|(d, u, t)| {
            serde_json::json!([d.format("%m-%d").to_string(), u, t])
        })
        .collect::<Vec<_>>())
}

/// 做种健康度：活种率（有做种者的过审种占比）+ 死种数 + 平均做种人数
async fn health(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> serde_json::Value {
    let (alive, dead, avg_seeders): (i64, i64, f64) = sqlx::query_as(
        "SELECT \
            count(*) FILTER (WHERE seeders > 0), \
            count(*) FILTER (WHERE seeders = 0), \
            COALESCE(avg(seeders), 0)::float8 \
         FROM torrents WHERE approval_status = 1",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or((0, 0, 0.0));
    let total = alive + dead;
    let rate = if total > 0 {
        (alive as f64 / total as f64 * 10000.0).round() / 100.0
    } else {
        0.0
    };
    serde_json::json!({
        "alive": alive, "dead": dead, "alive_rate": rate,
        "avg_seeders": (avg_seeders * 100.0).round() / 100.0,
    })
}

/// 周活：7 天内有活跃足迹（last_seen_at）的用户数
async fn wau(state: &web::Data<std::sync::Arc<AppState>>) -> i64 {
    sqlx::query_scalar(
        "SELECT count(DISTINCT id) FROM users \
         WHERE last_seen_at > now() - interval '7 days'",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0)
}

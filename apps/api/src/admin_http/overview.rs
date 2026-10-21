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
            (SELECT count(*) FROM users WHERE status < 2), \
            (SELECT count(*) FROM torrents), \
            (SELECT count(*) FROM users WHERE status >= 2)",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let users_total: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        // P-32：`users` 原本是无状态过滤的 count(*)，与同屏的 `banned_users`
        // 相加才等于它，读起来像「551 人里 457 人被封」；而对外页脚用
        // `status < 2`（同一库实测 94）。现主数改回在册成员，另出 `users_total`
        // 保留总量口径（users + banned_users == users_total，三者自洽）。
        "pending_reviews": pending, "open_reports": reports,
        "users": users, "users_total": users_total,
        "torrents": torrents, "banned_users": banned,
        "operator": auth.id,
        // E16 数据大盘三块：14 天注册/发种趋势（今日倒排）、做种健康度
        // （活种率/死种数）、周活（7 天内有 announce 或登录的用户数）。
        // 全部子查询点查级；空库/新站各值自然为 0，不做特殊分支。
        "trend": trend(&state).await,
        "health": health(&state).await,
        "wau": wau(&state).await,
    })))
}

/// 近 14 天注册/发种趋势：数组 [日期, 注册数, 发种数]，旧→新。
/// 日界按**站点日（UTC+8）**：旧写法 `generate_series(current_date-13, …)` 配
/// `created_at::date`（本库会话时区 UTC）画的是「北京 08:00→次日 08:00」的格子，
/// 而首页/会员中心的所有「今日」都是北京零点起算（P-465 同族）⇒ 站长在大盘
/// 看到的今日那一格少了整段北京 0–8 点，且全部堆到昨日。
async fn trend(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> serde_json::Value {
    let rows: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT (d AT TIME ZONE 'Asia/Shanghai')::date, \
            (SELECT count(*) FROM users u WHERE u.created_at >= d \
             AND u.created_at < d + interval '1 day'), \
            (SELECT count(*) FROM torrents t WHERE t.created_at >= d \
             AND t.created_at < d + interval '1 day') \
         FROM generate_series( \
            date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
              AT TIME ZONE 'Asia/Shanghai' - interval '13 days', \
            date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
              AT TIME ZONE 'Asia/Shanghai', \
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

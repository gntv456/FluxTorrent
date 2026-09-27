//! C5 经济反通胀运营面板（0226）：周粒度产出/消耗 + 池子 + Top SKU + 人均持币。
//!
//! 与既有 `/admin/spark-flow`（月度对账，v_spark_flow_monthly）互补：本端点给
//! 「本周/近 30 天」运营视角——站长看净通胀、动阀门（economy_max_buffer_gb）、
//! 下周再看的闭环。数据全部来自 spark_ledger / shop_orders / users 既有表。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/admin/economy-dashboard")]
pub(super) async fn economy_dashboard(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    // 近 8 周产出/消耗/净增发（按自然周对齐，站点时区）
    let weeks: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT to_char(date_trunc('week', created_at \
             AT TIME ZONE 'Asia/Shanghai'), 'MM-DD') AS wk, \
             COALESCE(SUM(amount) FILTER (WHERE amount > 0), 0)::bigint, \
             COALESCE(SUM(-amount) FILTER (WHERE amount < 0), 0)::bigint, \
             COALESCE(SUM(amount), 0)::bigint \
         FROM spark_ledger \
         WHERE created_at > now() - interval '56 days' \
         GROUP BY date_trunc('week', created_at AT TIME ZONE \
             'Asia/Shanghai') \
         ORDER BY date_trunc('week', created_at AT TIME ZONE \
             'Asia/Shanghai')",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站免池存量 + 全站持币 + 活跃用户（30 天登录）人均
    let pool: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM pool_donations",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let (total_spark, holders, active30): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(spark_balance), 0)::bigint, \
             COUNT(*)::bigint, \
             COUNT(*) FILTER (WHERE last_seen_at > now() - interval \
                 '30 days')::bigint \
             FROM users WHERE status < 2",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 近 30 天 Top 消耗 SKU（shop_orders 按商品聚合）
    let top_sku: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT si.name, COUNT(o.id)::bigint, \
             COALESCE(SUM(o.price), 0)::bigint \
         FROM shop_orders o JOIN shop_items si ON si.id = o.item_id \
         WHERE o.created_at > now() - interval '30 days' \
         GROUP BY si.name \
         ORDER BY SUM(o.price) DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 阀门现值（0 = 未启用）
    let cap_gb: f64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'economy_max_buffer_gb')::float8, 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0.0);
    Ok(ok(serde_json::json!({
        "weeks": weeks.iter().map(|(w, m, b, n)| serde_json::json!({
            "week": w, "minted": m, "burned": b, "net": n,
        })).collect::<Vec<_>>(),
        "pool_balance": pool,
        "total_spark": total_spark,
        "holders": holders,
        "active_30d": active30,
        "per_active_capita": if active30 > 0 {
            total_spark / active30
        } else { 0 },
        "top_sku_30d": top_sku.iter().map(|(n, c, s)| serde_json::json!({
            "name": n, "orders": c, "spent": s,
        })).collect::<Vec<_>>(),
        "purchase_cap_gb": cap_gb,
    })))
}

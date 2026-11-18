//! 契约运营视图（0499，体检二轮 B1）：站长侧签约/达标/罚金总览。
//!
//! 此前 admin 无任何 seed_contracts 读口——站长想知道「多少人签约、达标率、
//! 罚金总额」只能直连 DB。本端点聚合三块：当月签约概览、近六月结算走势、
//! 档位分布。权限与 economy_dashboard 同（SETTINGS_VIEW 运营视角）。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/admin/contract-stats")]
pub(super) async fn contract_stats(
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
    // 1) 当月签约概览（open=在约；enabled 档位口径）
    let (open_seed, open_publish, eligible_users): (i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
                 (SELECT count(*) FROM seed_contracts \
                  WHERE kind='seed' AND status='open' \
                    AND month = date_trunc('month', now())::date)::bigint, \
                 (SELECT count(*) FROM seed_contracts \
                  WHERE kind='publish' AND status='open' \
                    AND month = date_trunc('month', now())::date)::bigint, \
                 (SELECT count(*) FROM users \
                  WHERE created_at < now() - interval '30 days' \
                    AND status < 2)::bigint",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 2) 近六月结算走势（settled/breached/奖罚额）
    let months: Vec<(chrono::NaiveDate, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT month, \
                 count(*) FILTER (WHERE status='settled')::bigint, \
                 count(*) FILTER (WHERE status='breached')::bigint, \
                 COALESCE(sum(reward),0)::bigint, \
                 COALESCE(sum(penalty),0)::bigint \
             FROM seed_contracts \
             WHERE status IN ('settled','breached') \
               AND month > date_trunc('month', now())::date \
                           - interval '6 months' \
             GROUP BY month ORDER BY month DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 3) 档位分布（当月 open，按 tier 目标聚）
    let tiers: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT c.kind || ':' || c.target::text, c.target, \
                count(*)::bigint, \
                COALESCE(sum(c.target),0)::bigint \
         FROM seed_contracts c \
         WHERE c.status='open' \
           AND c.month = date_trunc('month', now())::date \
         GROUP BY c.kind, c.target ORDER BY c.kind, c.target",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "current": {
            "open_seed": open_seed,
            "open_publish": open_publish,
            "eligible_users": eligible_users,
            "seed_rate": if eligible_users > 0 {
                (open_seed as f64 / eligible_users as f64 * 100.0)
                    .round() as i64
            } else { 0 },
        },
        "months": months.iter().map(|(m, s, b, r, p)| serde_json::json!({
            "month": m.format("%Y-%m").to_string(),
            "settled": s, "breached": b,
            "reward": r, "penalty": p,
        })).collect::<Vec<_>>(),
        "tiers": tiers.iter().map(|(k, t, n, _)| serde_json::json!({
            "kind": k.split(':').next().unwrap_or(""),
            "target": t, "signers": n,
        })).collect::<Vec<_>>(),
        "enabled": sqlx::query_scalar::<_, bool>(
            "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name='seed_contract_enabled')='yes', true)",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(true),
    })))
}

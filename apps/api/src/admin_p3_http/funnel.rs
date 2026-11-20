//! 生命周期 cohort 漏斗（0505 运营体检批）：GET /admin/funnel。
//! 运营此前无法回答「本周/近 N 周注册的用户有多少完成了首次下载/
//! 首次做种/首次签到」——onboarding_facts 只按单人实时判定，注册
//! 趋势（admin/overview trend）只有注册/发种计数不含转化。本口按
//! 注册周 cohort 聚合四步漏斗：注册→首次下载→首次做种→首次签到。
//! 数据源 users × snatches × attendance；snatches 无「开始做种时刻」
//! 列，做种步按事实判定（seeded_seconds>0 存在即达成）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct FunnelQ {
    /// 回看周数（1-26，缺省 8）
    #[serde(default)]
    pub weeks: Option<i32>,
}

/// GET /admin/funnel?weeks=N —— 注册 cohort 生命周期漏斗（SETTINGS_VIEW）。
#[get("/admin/funnel")]
pub async fn funnel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FunnelQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let weeks = q.weeks.unwrap_or(8).clamp(1, 26);
    // 按 date_trunc('week')（周一为界）分 cohort：
    //   registered  = 该周注册且未注销人数
    //   first_dl    = 有 downloaded>0 的 snatch（完成下载）
    //   first_seed  = 有 seeded_seconds>0 的 snatch（做过种）
    //   first_ck    = attendance 有任一签到行
    // 三步都是 EXISTS 事实判定，不依赖各自的时间先后。
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        r#"
        WITH cohorts AS (
            SELECT date_trunc('week', created_at)::date AS wk,
                   count(*)::bigint AS registered
            FROM users
            WHERE created_at >= date_trunc('week', now())
                    - make_interval(weeks => $1)
              AND status < 2
            GROUP BY 1
        )
        SELECT c.wk::text,
               c.registered,
               count(fd.uid)::bigint,
               count(fs.uid)::bigint,
               count(fc.uid)::bigint
        FROM cohorts c
        LEFT JOIN (
            SELECT DISTINCT u.id AS uid
            FROM users u
            JOIN snatches s ON s.user_id = u.id AND s.downloaded > 0
            WHERE u.created_at >= date_trunc('week', now())
                    - make_interval(weeks => $1)
        ) fd ON TRUE
        LEFT JOIN LATERAL (
            SELECT 1 AS uid WHERE EXISTS (
                SELECT 1 FROM snatches s
                WHERE s.user_id = fd.uid AND s.seeded_seconds > 0)
        ) fs ON TRUE
        LEFT JOIN LATERAL (
            SELECT 1 AS uid WHERE EXISTS (
                SELECT 1 FROM attendance a WHERE a.user_id = fd.uid)
        ) fc ON TRUE
        GROUP BY c.wk, c.registered
        ORDER BY c.wk DESC
        "#,
    )
    .bind(weeks as i32)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|(wk, reg, dl, seed, ck)| {
            serde_json::json!({
                "week": wk,
                "registered": reg,
                "firstDownload": dl,
                "firstSeed": seed,
                "firstCheckin": ck,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "weeks": weeks, "rows": out })))
}

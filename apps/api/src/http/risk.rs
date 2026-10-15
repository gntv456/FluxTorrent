//! 反作弊风险评分查询 M2（0368）：观察名单 + 用户维度构成。
//! 分数由 worker/jobs/risk_score.rs 周期重算落 user_risk_scores——
//! 本模块只读，公式唯一权威在 worker（两处实现必然漂移）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::auth_infra::{require_auth, require_staff};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
struct RiskQuery {
    /// 档位精确过滤：none/low/watch/high/critical；缺省/all = 全部
    #[serde(default)]
    band: Option<String>,
    /// 用户名模糊（管理端找人）
    #[serde(default)]
    q: Option<String>,
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct RiskRow {
    user_id: i64,
    username: String,
    score: i32,
    band: String,
    ruleset_ver: i32,
    updated_at: chrono::DateTime<chrono::Utc>,
    open_case: Option<i64>,
    live_evidence: i64,
    live_hits: i64,
    shadow_breakdown: Option<serde_json::Value>,
}

/// 观察名单/全集 SELECT（band/用户名过滤用 NULL 卫句摊平动态条件）。
/// open_case/live_* 三个标量子查询与 cheat_case_list 同口径。
const RISK_SELECT: &str = "\
SELECT r.user_id, u.username, r.score, r.band, r.ruleset_ver, r.updated_at, \
       (SELECT p.id FROM reports p WHERE p.ref_type = 'cheat' \
         AND p.ref_id = r.user_id AND p.status IN (0, 2)) AS open_case, \
       (SELECT count(*) FROM cheat_events c \
         WHERE c.user_id = r.user_id AND c.resolved_at IS NULL) \
         AS live_evidence, \
       (SELECT COALESCE(sum(c.hits), 0)::bigint FROM cheat_events c \
         WHERE c.user_id = r.user_id AND c.resolved_at IS NULL) \
         AS live_hits, \
       r.shadow_breakdown \
FROM user_risk_scores r \
JOIN users u ON u.id = r.user_id";

/// GET /admin/risk-scores：观察名单（score 倒序，watch+ 即入名单）。
#[get("/admin/risk-scores")]
pub async fn risk_score_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<RiskQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let band: Option<String> = match q.band.as_deref() {
        None | Some("") | Some("all") => None,
        Some(b @ ("none" | "low" | "watch" | "high" | "critical")) => {
            Some(b.to_string())
        }
        Some(other) => {
            return Err(DomainError::Validation(format!(
                "band 只认 none/low/watch/high/critical/all：{other}"
            )))
        }
    };
    let like = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));
    let rows = sqlx::query_as::<_, RiskRow>(&format!(
        "{RISK_SELECT} \
         WHERE ($1::text IS NULL OR r.band = $1) \
           AND ($2::text IS NULL OR u.username LIKE $2) \
         ORDER BY r.score DESC, r.updated_at DESC LIMIT $3"
    ))
    .bind(band)
    .bind(like)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "items": rows })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct EvidenceRow {
    agent: String,
    reason: String,
    hits: i64,
    first_seen: chrono::DateTime<chrono::Utc>,
    last_seen: chrono::DateTime<chrono::Utc>,
    case_id: Option<i64>,
}

/// GET /admin/users/{id}/risk：用户维度分数构成 + 存活证据明细。
/// 无分数行（从未有证据/已被清掉）时回落 0 分 none 档，不 404——
/// 管理端点开任意用户都应看到「干净」而非报错。
#[get("/admin/users/{id}/risk")]
pub async fn user_risk_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let uid = path.into_inner();
    let uname: Option<String> =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(username) = uname else {
        return Err(DomainError::NotFound(uid));
    };
    let row = sqlx::query_as::<_, RiskRow>(&format!(
        "{RISK_SELECT} WHERE r.user_id = $1"
    ))
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let evidence = sqlx::query_as::<_, EvidenceRow>(
        "SELECT agent, reason, hits, first_seen, last_seen, case_id \
         FROM cheat_events WHERE user_id = $1 AND resolved_at IS NULL \
         ORDER BY hits DESC, last_seen DESC LIMIT 100",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 无分数行 → 零值信封（breakdown 为 null 表示「未算过」而非「算过是 0」）
    let (score, band, ver, updated, breakdown, open_case) = match row {
        Some(r) => (
            r.score,
            r.band,
            r.ruleset_ver,
            Some(r.updated_at),
            r.shadow_breakdown,
            r.open_case,
        ),
        None => (0, "none".to_string(), 0, None, None, None),
    };
    Ok(ok(serde_json::json!({
        "user_id": uid,
        "username": username,
        "score": score,
        "band": band,
        "ruleset_ver": ver,
        "updated_at": updated,
        "shadow_breakdown": breakdown,
        "open_case": open_case,
        "evidence": evidence,
    })))
}

/// 路由挂载（同 cheat_case/mount.rs 模式：http/mod.rs 守 300 行门禁）。
pub fn mount_risk(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(risk_score_list).service(user_risk_detail)
}

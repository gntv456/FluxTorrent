//! 体育对阵实体（0330 sports 批）：admin 维护 + 用户读面 + 发种挂链。
//!
//! 承载决策见迁移 0330：对阵是「一行一赛」的事实（比分/主客/时间），
//! 维度撑不起；league/season/round 检索继续走维度，比分与对阵页挂本表。
//! admin 建/改比分 → 用户按队/轮筛 → 对阵页并列该场全部资源
//! （全场/集锦/回放）→ 发种 match_id 挂链（自由字段，artifact parent 范式）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ admin 维护面 ============

#[derive(Deserialize)]
struct MatchBody {
    league: String,
    #[serde(default)]
    season: String,
    #[serde(default)]
    round: String,
    home: String,
    away: String,
    #[serde(default)]
    home_score: Option<i32>,
    #[serde(default)]
    away_score: Option<i32>,
    #[serde(default)]
    kickoff: Option<String>,
}

fn parse_ts(v: &Option<String>) -> DomainResult<Option<chrono::DateTime<chrono::Utc>>> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    DomainError::Validation(
                        "kickoff 需为 RFC3339（如 2026-10-11T19:30:00Z）".into(),
                    )
                })
        })
        .transpose()
}

/// 建赛（staff，TORRENT_REVIEW 权限口径——比分是站点公信数据）。
#[post("/admin/matches")]
pub async fn admin_match_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MatchBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let b = body.into_inner();
    if b.league.trim().is_empty() || b.home.trim().is_empty()
        || b.away.trim().is_empty()
    {
        return Err(DomainError::Validation(
            "联赛/主队/客队不能为空".into(),
        ));
    }
    if b.home.trim() == b.away.trim() {
        return Err(DomainError::Validation("主客队不能相同".into()));
    }
    let kickoff = parse_ts(&b.kickoff)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sport_matches (league, season, round, home, away, \
         home_score, away_score, kickoff) VALUES ($1, $2, $3, $4, $5, \
         $6, $7, $8) ON CONFLICT (league, season, round, home, away) DO \
         UPDATE SET home_score = EXCLUDED.home_score, away_score = \
         EXCLUDED.away_score, kickoff = EXCLUDED.kickoff RETURNING id",
    )
    .bind(b.league.trim())
    .bind(b.season.trim())
    .bind(b.round.trim())
    .bind(b.home.trim())
    .bind(b.away.trim())
    .bind(b.home_score)
    .bind(b.away_score)
    .bind(kickoff)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "sport_match_upsert", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// 比分补录（赛后改分）。
#[put("/admin/matches/{id}/score")]
pub async fn admin_match_score(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<serde_json::Value>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let id = path.into_inner();
    let b = body.into_inner();
    let hs = b.get("home_score").and_then(|v| v.as_i64());
    let as_ = b.get("away_score").and_then(|v| v.as_i64());
    let n = sqlx::query(
        "UPDATE sport_matches SET home_score = $2, away_score = $3 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(hs.map(|v| v as i32))
    .bind(as_.map(|v| v as i32))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "sport_match_score", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "updated": id })))
}

// ============ 用户读面 ============

#[derive(Deserialize)]
struct MatchQuery {
    #[serde(default)]
    league: Option<String>,
    #[serde(default)]
    season: Option<String>,
    #[serde(default)]
    round: Option<String>,
    /// 队名（主或客命中）
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
}

fn match_row(
    m: &serde_json::Value,
    torrents: i64,
) -> serde_json::Value {
    let mut v = m.clone();
    if let Some(o) = v.as_object_mut() {
        o.insert("torrents".into(), serde_json::json!(torrents));
    }
    v
}

/// 对阵列表（登录可读）：按联赛/赛季/轮次/队筛，近赛在前。
#[get("/matches")]
pub async fn matches_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MatchQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT to_jsonb(m) FROM sport_matches m \
         WHERE ($1::text IS NULL OR m.league = $1) \
           AND ($2::text IS NULL OR m.season = $2) \
           AND ($3::text IS NULL OR m.round = $3) \
           AND ($4::text IS NULL OR m.home ILIKE '%' || $4 || '%' \
                OR m.away ILIKE '%' || $4 || '%') \
         ORDER BY m.kickoff DESC NULLS LAST, m.id DESC LIMIT $5",
    )
    .bind(q.league.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(q.season.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(q.round.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(q.team.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 各场资源数（对阵页入口的角标）
    let ids: Vec<i64> = rows
        .iter()
        .filter_map(|(m,)| m.get("id").and_then(|v| v.as_i64()))
        .collect();
    let mut counts: std::collections::HashMap<i64, i64> =
        std::collections::HashMap::new();
    if !ids.is_empty() {
        let cs: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT match_id, count(*) FROM torrents \
             WHERE match_id = ANY($1) AND approval_status = 1 \
             GROUP BY match_id",
        )
        .bind(&ids)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        counts = cs.into_iter().collect();
    }
    let items: Vec<_> = rows
        .iter()
        .map(|(m,)| {
            let id = m.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match_row(m, counts.get(&id).copied().unwrap_or(0))
        })
        .collect();
    Ok(ok(items))
}

/// 对阵页：该场全部过审种（全场/集锦/回放并列）。
#[get("/matches/{id}")]
pub async fn match_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let m: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT to_jsonb(m) FROM sport_matches m WHERE m.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((m,)) = m else {
        return Err(DomainError::NotFound(id));
    };
    let items: Vec<(i64, String, i64, i32, i32)> = sqlx::query_as(
        "SELECT id, name, size, seeders, times_completed FROM torrents \
         WHERE match_id = $1 AND approval_status = 1 \
         ORDER BY id DESC LIMIT 100",
    )
    .bind(id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "match": m,
        "items": items.iter().map(|(tid, n, sz, sd, tc)| {
            serde_json::json!({
                "id": tid, "name": n, "size": sz,
                "seeders": sd, "times_completed": tc,
            })
        }).collect::<Vec<_>>(),
    })))
}

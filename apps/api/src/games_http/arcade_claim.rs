//! 周常 / 赛季里程碑领取：幂等落 arcade_claims + earn_spark 入账。
//! 领取是「确定侧发放」，走 arcade_budget_* 的预算闸（见 arcade_meta 的门禁）；
//! 幂等键含 period_key（周常 '2026-W40' / 赛季 'S1'），重复提交与重跑安全。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::arcade_cfg::{MILESTONES, QUESTS, SEASON_KEY};

#[derive(Deserialize)]
pub(super) struct ClaimReq {
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

/// ISO 周键（与 arcade_meta 同口径）
const WEEK_SQL: &str = "SELECT to_char(now(), 'IYYY-\"W\"IW')";

/// 领取周常奖励：校验本周进度 → 幂等落 claim → 入账
#[post("/games/arcade/quest/{code}/claim")]
pub(super) async fn claim_quest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let code = path.into_inner();
    let (_, game, target, reward) = QUESTS
        .iter()
        .find(|q| q.0 == code)
        .copied()
        .ok_or_else(|| DomainError::Validation("无此周常".into()))?;
    let db = &state.repo.db;
    let week: String = sqlx::query_scalar(WEEK_SQL)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let done: i64 = if game == "*" {
        sqlx::query_scalar(
            "SELECT count(*)::bigint FROM spark_ledger \
             WHERE user_id = $1 AND kind = 'game' AND amount < 0 \
               AND created_at >= date_trunc('week', now())",
        )
        .bind(uid)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        sqlx::query_scalar(
            "SELECT count(*)::bigint FROM spark_ledger \
             WHERE user_id = $1 AND kind = 'game' AND amount < 0 \
               AND ref_type = $2 \
               AND created_at >= date_trunc('week', now())",
        )
        .bind(uid)
        .bind(game)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    };
    if done < target {
        return Err(DomainError::Validation("本周进度未达领取条件".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO arcade_claims (user_id, kind, ref_code, period_key) \
         VALUES ($1, 'quest', $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(uid)
    .bind(&code)
    .bind(&week)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if ins.rows_affected() == 0 {
        return Err(DomainError::Validation("本周已领取".into()));
    }
    let idem = match &body.idempotency_key {
        Some(k) if !k.trim().is_empty() && k.len() <= 128 => {
            format!("arcade:quest:{code}:{week}:{uid}:{k}")
        }
        _ => format!("arcade:quest:{code}:{week}:{uid}"),
    };
    earn_spark(db, uid, reward, "arcade", &idem).await?;
    Ok(ok(serde_json::json!({ "reward": reward })))
}

/// 领取赛季里程碑奖励：校验票根数 → 幂等落 claim → 入账
#[post("/games/arcade/season/{code}/claim")]
pub(super) async fn claim_season(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let code = path.into_inner();
    let (_, need, reward) = MILESTONES
        .iter()
        .find(|m| m.0 == code)
        .copied()
        .ok_or_else(|| DomainError::Validation("无此里程碑".into()))?;
    let db = &state.repo.db;
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM user_achievements ua \
         JOIN achievement_defs d ON d.id = ua.def_id \
         WHERE ua.user_id = $1 AND d.family = 'arcade'",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if owned < need {
        return Err(DomainError::Validation("票根数未达里程碑".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO arcade_claims (user_id, kind, ref_code, period_key) \
         VALUES ($1, 'season', $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(uid)
    .bind(&code)
    .bind(SEASON_KEY)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if ins.rows_affected() == 0 {
        return Err(DomainError::Validation("该里程碑已领取".into()));
    }
    let idem = match &body.idempotency_key {
        Some(k) if !k.trim().is_empty() && k.len() <= 128 => {
            format!("arcade:season:{SEASON_KEY}:{code}:{uid}:{k}")
        }
        _ => format!("arcade:season:{SEASON_KEY}:{code}:{uid}"),
    };
    earn_spark(db, uid, reward, "arcade", &idem).await?;
    Ok(ok(serde_json::json!({ "reward": reward })))
}

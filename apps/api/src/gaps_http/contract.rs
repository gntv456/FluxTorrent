//! 月度契约用户侧（0424，同类站对标 A2）。
//!
//! 三个口：档位列表（签约前看价目）、签约/续约（当前月）、我的契约（历史+当月）。
//! 豁免语义在读侧：当月存在 open/settled 契约即豁免活跃度考核（见
//! `services/contract_status`；worker 结算在 `worker/src/jobs/contract.rs`）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::Datelike;
use serde::Deserialize;
use sqlx::FromRow;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(FromRow, serde::Serialize)]
struct ContractRow {
    id: i64,
    kind: String,
    month: chrono::NaiveDate,
    target: i64,
    achieved: i64,
    status: String,
    reward: i32,
    penalty: i32,
    auto_renew: bool,
    missed_streak: i32,
}

/// 当月月首日（UTC 口径，与 worker 结算的 month 同源）。
fn current_month() -> chrono::NaiveDate {
    let today = chrono::Utc::now().date_naive();
    today.with_day(1).expect("day 1 always valid")
}

/// GET /contracts/tiers —— 档位价目（登录可读；签约资格一并返回）。
#[get("/contracts/tiers")]
pub async fn contract_tiers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'seed_contract_enabled') = 'yes', true)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(true);
    let tiers: Vec<(String, String, i64, i32, i32, i32)> = sqlx::query_as(
        "SELECT kind, label, target, reward, penalty, sort \
         FROM seed_contract_tiers WHERE enabled \
         ORDER BY kind, sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 资格：注册满 30 天（同类站 口径）
    let eligible: bool = sqlx::query_scalar(
        "SELECT created_at < now() - interval '30 days' FROM users \
         WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(false);
    Ok(ok(serde_json::json!({
        "enabled": enabled,
        "eligible": eligible,
        "tiers": tiers
            .iter()
            .map(|(kind, label, target, reward, penalty, sort)| {
                serde_json::json!({
                    "kind": kind, "label": label, "target": target,
                    "reward": reward, "penalty": penalty, "sort": sort,
                })
            })
            .collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct SignReq {
    kind: String,
    target: i64,
    #[serde(default)]
    auto_renew: bool,
}

/// POST /contracts/sign —— 签约当前月（或切换自动续约）。
///
/// 幂等：同 (user, month, kind) 已有行时仅更新 auto_renew，不换档——
/// 换档会造成「先签低档再升高档」的套利窗口（月内进度只涨不降）。
#[post("/contracts/sign")]
pub async fn contract_sign(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SignReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.kind != "seed" && body.kind != "publish" {
        return Err(DomainError::Validation(
            "契约类型必须是 seed/publish".into(),
        ));
    }
    // 档位必须真实存在且启用（不接受任意 target）
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM seed_contract_tiers \
         WHERE kind = $1 AND target = $2 AND enabled)",
    )
    .bind(&body.kind)
    .bind(body.target)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !valid {
        return Err(DomainError::Validation("契约档位不存在或未启用".into()));
    }
    let eligible: bool = sqlx::query_scalar(
        "SELECT created_at < now() - interval '30 days' FROM users \
         WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(false);
    if !eligible {
        return Err(DomainError::Validation(
            "注册满 30 天后才可参与契约".into(),
        ));
    }
    let month = current_month();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO seed_contracts (user_id, kind, month, target, auto_renew) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, month, kind) \
         DO UPDATE SET auto_renew = EXCLUDED.auto_renew \
         RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.kind)
    .bind(month)
    .bind(body.target)
    .bind(body.auto_renew)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "contract.sign", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "contract_id": id, "month": month })))
}

/// GET /contracts/mine —— 我的契约（当月在前，倒序）。
#[get("/contracts/mine")]
pub async fn contract_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<ContractRow> = sqlx::query_as(
        "SELECT id, kind, month, target, achieved, status, reward, penalty, \
         auto_renew, missed_streak FROM seed_contracts \
         WHERE user_id = $1 ORDER BY month DESC, kind LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

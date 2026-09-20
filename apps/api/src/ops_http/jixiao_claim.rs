//! 绩效奖金领取与历史（M21）：claim + my。
//! 从 ops_http/jixiao.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use super::jixiao::{jixiao_bonus, jixiao_prev_period, jixiao_setting_i64};
use super::jixiao_compute::compute_metrics;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/jixiao/claim")]
pub(super) async fn jixiao_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let now_period = chrono::Utc::now().format("%Y-%m").to_string();
    let period = body.period.clone().unwrap_or(now_period.clone());

    // 补领窗口：只能领当月或上月（且上月必须在窗口期内；月末已自动结算过的不可再领）
    if period != now_period {
        if Some(period.clone()) != jixiao_prev_period(&now_period) {
            return Err(DomainError::Validation(
                "只能领取本期或上一期的绩效工资".into(),
            ));
        }
        let window =
            jixiao_setting_i64(&state.repo.db, "jixiao_claim_window_days", 7)
                .await;
        let elapsed = days_after_period_end(&period);
        if elapsed < 0 {
            return Err(DomainError::Validation("该期尚未结束".into()));
        }
        if elapsed > window {
            return Err(DomainError::Validation(format!(
                "补领窗口（{window} 天）已过"
            )));
        }
    }

    // 审计修复（P0 无上限铸币）：考核工资必须由 admin 经 /admin/users/{id}/jixiao
    // 分配岗位后才能领取。旧逻辑任何登录用户可自选任意 type_id 领取（min_requirements
    // 全站为空使校验空转），每用户每月可扫全部岗位 ≈89,500 火花。
    let assigned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims \
         WHERE user_id = $1 AND type_id = $2 AND period = $3 \
           AND metrics_snapshot->>'source' = 'admin')",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !assigned {
        return Err(DomainError::Validation(
            "该期管理组尚未为你分配此考核岗位，无法领取".into(),
        ));
    }

    // 幂等：本期已结算（登记行已推进到 status=1）不可再领；
    // worker 月末自动结算与本人补领是**同一行**的 CAS 互斥（0106 单行设计）
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims \
         WHERE user_id = $1 AND type_id = $2 AND period = $3 \
           AND metrics_snapshot->>'source' = 'admin' AND settled_at IS NOT NULL)",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if claimed {
        return Err(DomainError::Validation(
            "本期工资已发放（自动结算或本人领取）".into(),
        ));
    }

    let t: Option<(String, i64, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT name, base_pay, metrics, min_requirements FROM jixiao_types WHERE id = $1",
    )
    .bind(body.type_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, base_pay, _metrics, min_reqs)) = t else {
        return Err(DomainError::NotFound(body.type_id));
    };

    let metrics = compute_metrics(&state.repo.db, auth.id, &period).await?;
    // 最低要求线校验（低于要求不发基本工资 —— 旧站口径）
    for (key, min) in min_reqs.as_object().unwrap_or(&serde_json::Map::new()) {
        let actual = metrics.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let required = min.as_i64().unwrap_or(0);
        if required > 0 && actual < required {
            return Err(DomainError::Validation(format!(
                "「{name}」考核未达标：{key} 需 {required}，实际 {actual}"
            )));
        }
    }
    // 达标月数加成：每累计 N 个达标月 +P%（配置化，旧实现硬编码 /10*(m/3)）
    // 达标月数（**含本期**：历史 status=1 行数 + 1，本期达标正在领取）。
    // worker 结算侧同口径（months_before + 1）——首月即计入加成档位。
    let qualified_months: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND status = 1",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0)
        + 1;
    let bonus =
        jixiao_bonus(&state.repo.db, body.type_id, base_pay, qualified_months)
            .await;
    let total = base_pay + bonus;

    // 单行设计（0106 修正）：登记与发放是同一行的状态流转——
    // UNIQUE(user_id, type_id, period) 下不可能同时存在 admin 登记行和用户领取行，
    // 旧实现「登记行 + 再 INSERT 领取行」必然撞唯一约束（表现为永远「本期已领取过工资」）。
    // 领取 = 把登记行 CAS 推进到 status=1（与 worker 结算同一互斥口径）。
    let claim_id: i64 = sqlx::query_scalar(
        "UPDATE jixiao_claims SET status = 1, settled_at = now(), amount = $3, bonus_paid = $4, \
                metrics_at_settle = $5, \
                metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"self\"}'::jsonb \
         WHERE user_id = $1 AND type_id = $2 AND period = $6 \
           AND metrics_snapshot->>'source' = 'admin' AND settled_at IS NULL AND status = 0 \
         RETURNING id",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(total)
    .bind(bonus)
    .bind(metrics.clone())
    .bind(&period)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or_else(|| DomainError::Validation("本期已领取过工资或已由系统结算".into()))?;

    let idem = format!("jixiao:{}:{}", claim_id, period);
    // kind 修正：旧实现误复用 task_reward（报表与任务奖励混同）
    earn_spark(&state.repo.db, auth.id, total, "jixiao_reward", &idem).await?;
    state
        .repo
        .audit(Some(auth.id), "jixiao_claim", Some(claim_id))
        .await;
    Ok(ok(
        serde_json::json!({ "claim_id": claim_id, "type": name, "base": base_pay, "bonus": bonus, "total": total }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ClaimRow {
    id: i64,
    type_name: String,
    period: String,
    amount: i64,
    #[serde(default)]
    bonus_paid: i64,
    #[serde(default)]
    status: i16,
    /// 发放来源：worker=月末自动结算 / self=本人领取（展示「自动发放/手动领取」）
    #[sqlx(default)]
    source: Option<String>,
    claimed_at: chrono::DateTime<chrono::Utc>,
}

#[get("/jixiao/my")]
pub(super) async fn jixiao_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.id, t.name AS type_name, c.period, c.amount, c.bonus_paid, c.status, \
                c.metrics_snapshot->>'settle_by' AS source, c.claimed_at \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.user_id = $1 AND c.status = 1 \
         ORDER BY c.settled_at DESC NULLS LAST, c.id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M22 任务中心 ============

#[derive(Deserialize)]
struct ClaimReq {
    type_id: i64,
    /// YYYY-MM；缺省当月。允许上一期（月末结算后补领窗口内）
    #[serde(default)]
    period: Option<String>,
}

/// 站点时区（UTC+8）下的今天与 period 月末之间的天数差（正数 = period 已结束 N 天）
fn days_after_period_end(period: &str) -> i64 {
    let (y, m): (i32, u32) = {
        let mut it = period.split('-');
        let y = it.next().and_then(|v| v.parse().ok()).unwrap_or(2026);
        let m = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
        (y, m)
    };
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    use chrono::TimeZone;
    let month_end = chrono::FixedOffset::east_opt(8 * 3600)
        .and_then(|tz| tz.with_ymd_and_hms(ny, nm, 1, 0, 0, 0).single())
        .unwrap_or_else(|| {
            chrono::Utc::now().with_timezone(
                &chrono::FixedOffset::east_opt(8 * 3600).unwrap(),
            )
        });
    let now_cst = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
    (now_cst - month_end).num_days()
}

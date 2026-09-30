//! 优先级① 核心行为联动（轻量版）HTTP 接口：状态查询 + 口粮券核销。
//!
//! - `GET  /games/linkage/status`：现算口粮券余额 / 今日做种小时 / 本周做种小时 /
//!   今日是否已发卡 / 渔汛做种门槛阈值与达成状态。
//! - `POST /games/coupons/use`：核销 1 张口粮券（行锁 + 幂等键防双花），返回剩余。
//!   玩家侧消耗出口已接宠物投喂（`POST /games/pet/feed` 带 `use_coupon`）；
//!   本端点保留作为通用核销入口（鱼竿升级等后续 sink 直接调它）。
//!
//! 全部走 runtime `sqlx::query`（不引入编译期 sqlx 依赖），与 worker 侧口径一致。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::idem_key;

/// 行为联动状态：把「做种」行为换算成游戏资源进度，前端据此展示正循环。
/// 周做种小时与渔汛四态统一走 `fishing_extra`（与渔汛拦截同一份数据源，
/// 两处口径漂移 = 玩家看到「已解锁」却被拦）。
#[get("/games/linkage/status")]
pub(super) async fn linkage_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let row: (i64, bool) = sqlx::query_as(
        r#"
        SELECT
          COALESCE((SELECT food_coupons FROM users WHERE id = $1), 0)::bigint,
          EXISTS(SELECT 1 FROM food_coupon_grants g
                 WHERE g.user_id = $1
                   AND g.day = ((now() AT TIME ZONE 'UTC')
                     + interval '8 hours')::date)
        "#,
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 今日做种小时：seeding_reward 流水按小时幂等键去重（8:00 起算的一天）
    let daily: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT split_part(l.idempotency_key, ':', 3))::bigint \
           FROM spark_ledger l \
          WHERE l.kind = 'seeding_reward' AND l.user_id = $1 \
            AND l.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') \
              + interval '8 hours' \
            AND l.created_at <  date_trunc('day', now() AT TIME ZONE 'UTC') \
              + interval '32 hours'",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let (ev_on, ev_ok, weekly_hours, threshold) =
        super::fishing_extra::event_state(&state, auth.id).await?;

    Ok(ok(serde_json::json!({
        "food_coupons": row.0,
        "daily_seed_hours": daily,
        "weekly_seed_hours": weekly_hours,
        "coupon_granted_today": row.1,
        "fishing_event_threshold": threshold,
        "fishing_event_unlocked": ev_ok,
        "fishing_event_active": ev_on,
    })))
}

#[derive(Deserialize)]
pub(super) struct UseCouponReq {
    /// 客户端幂等键（网络重试 / 双击须带同一键，缺省回落随机键保持兼容）
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

/// 核销 1 张口粮券（行为联动资源的消耗入口）。
/// 安全模型：① `FOR UPDATE` 锁用户行防并发双花；② `coupon_uses` 幂等键防同键重放；
/// 两者配合，既堵「首刷未扣 + 重放已扣」白嫖，也堵两笔不同键并发各扣一次的双花。
#[post("/games/coupons/use")]
pub(super) async fn use_coupon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<UseCouponReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    let idem = idem_key("use_coupon", auth.id, &client_idem);

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let balance: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(food_coupons, 0)::bigint
               FROM users WHERE id = $1 FOR UPDATE"#,
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    if balance <= 0 {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("口粮券不足".into()));
    }

    // 同键重放：直接返回当前余额，不重复核销
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM coupon_uses WHERE idem = $1)",
    )
    .bind(&idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if seen {
        let _ = tx.rollback().await;
        return Ok(ok(serde_json::json!({
            "consumed": false,
            "reason": "duplicate",
            "food_coupons": balance,
        })));
    }

    // 登记幂等 + 扣减，同事务原子
    sqlx::query("INSERT INTO coupon_uses (idem, user_id) VALUES ($1, $2)")
        .bind(&idem)
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let new_balance: i64 = sqlx::query_scalar(
        "UPDATE users SET food_coupons = food_coupons - 1 WHERE id = $1 \
         RETURNING food_coupons::bigint",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "consumed": true,
        "food_coupons": new_balance,
    })))
}

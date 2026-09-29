//! 优先级① 核心行为联动（轻量版）HTTP 接口：状态查询 + 口粮券核销。
//!
//! - `GET  /games/linkage/status`：现算口粮券余额 / 今日做种小时 / 本周做种小时 /
//!   今日是否已发卡 / 渔汛做种门槛阈值与达成状态。
//! - `POST /games/coupons/use`：核销 1 张口粮券（行锁 + 幂等键防双花），返回剩余。
//!   供未来养成喂养 / 鱼竿升级消耗调用（当前宠物 / 钓鱼模块尚未落地）。
//!
//! 全部走 runtime `sqlx::query`（不引入编译期 sqlx 依赖），与 worker 侧口径一致。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{eco_i64, idem_key};

/// 行为联动状态：把「做种」行为换算成游戏资源进度，前端据此展示正循环。
#[get("/games/linkage/status")]
pub(super) async fn linkage_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let row: (i64, i64, i64, bool) = sqlx::query_as(
        r#"
        SELECT
          COALESCE((SELECT food_coupons FROM users WHERE id = $1), 0)::bigint,
          (SELECT count(DISTINCT split_part(l.idempotency_key, ':', 3))
             FROM spark_ledger l
             WHERE l.kind = 'seeding_reward'
               AND l.user_id = $1
               AND l.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC' + interval '8 hours')
               AND l.created_at <  date_trunc('day', now() AT TIME ZONE 'UTC' + interval '8 hours') + interval '1 day'),
          (SELECT count(DISTINCT split_part(l.idempotency_key, ':', 3))
             FROM spark_ledger l
             WHERE l.kind = 'seeding_reward'
               AND l.user_id = $1
               AND l.created_at >= date_trunc('week', now() AT TIME ZONE 'UTC' + interval '8 hours')),
          EXISTS(SELECT 1 FROM food_coupon_grants g
                 WHERE g.user_id = $1
                   AND g.day = (now() AT TIME ZONE 'UTC' + interval '8 hours')::date)
        "#,
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 渔汛门槛阈值走设置键（缺省 20 小时/周），运维可热调
    let weekly_threshold =
        eco_i64(&state, "games_fishing_event_seed_hours", 20).await;
    let fishing_event_unlocked = row.2 >= weekly_threshold;

    Ok(ok(serde_json::json!({
        "food_coupons": row.0,
        "daily_seed_hours": row.1,
        "weekly_seed_hours": row.2,
        "coupon_granted_today": row.3,
        "fishing_event_threshold": weekly_threshold,
        "fishing_event_unlocked": fishing_event_unlocked,
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
        "SELECT COALESCE(food_coupons, 0)::bigint FROM users WHERE id = $1 FOR UPDATE",
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

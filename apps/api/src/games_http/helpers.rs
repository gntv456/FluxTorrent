//! 游戏域共享助手：经济读数/赔率常量/每小时限次/幂等键。
//! 从 games_http.rs 按域拆出。

use actix_web::web;
use serde::Deserialize;
use uuid::Uuid;

use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_PLAYS_PER_HOUR};
use crate::state::AppState;

/// 读取游戏经济设置键（0109 参数化；缺省回落代码默认值 T3）
pub(super) async fn eco_i64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: i64,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         $1)::bigint, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 浮点设置键（赔率类支持小数，如猜大小 1.9x）
pub(super) async fn eco_f64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: f64,
) -> f64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         $1)::double precision, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 刮刮乐档位（五档可配；10x 留空/合计不为 100 时按余数推导，缺省 45/30/15/8/2）
pub(super) async fn scratch_odds(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> games::ScratchOdds {
    games::ScratchOdds::from_parts(
        eco_i64(state, "games_scratch_empty_pct", 45).await,
        eco_i64(state, "games_scratch_half_pct", 30).await,
        eco_i64(state, "games_scratch_one_pct", 15).await,
        eco_i64(state, "games_scratch_two_pct", 8).await,
        eco_i64(state, "games_scratch_ten_pct", 2).await,
    )
}

/// 农场作物有效期（天；0 = 永不枯萎）
pub(super) async fn farm_wither_days(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    eco_i64(state, "farm_wither_days", 5).await.clamp(0, 60)
}

/// 农场市场价刷新窗口（小时）。四审 L8：0109 登记了该键但无人读，站长改数字
/// 不生效；此处接线后窗口仍钳 1..24，保证它是 3600 的整数倍且不超过一天。
pub(super) async fn farm_market_hours(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    eco_i64(state, "farm_market_window_hours", 4)
        .await
        .clamp(1, 24)
}

/// 猜大小赔率（千分比）。倍数设置键缺省 1.9 —— **必须 < 2.0**：
/// 2.0 时 EV 恰为 1.0（不回收）且可双向零风险对冲，见 games.rs 常量说明。
/// 上限钳到 1999‰（运行时 EV 防线，P2）：设置键是管理员可写参数，此前上限 10_000‰
/// 意味着误配/越权写入 ≥2000‰ 即把游戏变成增发开关。
pub(super) async fn bigsmall_mult_permille(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    let mult = eco_f64(state, "games_bigsmall_win_mult", 1.9).await;
    ((mult * 1000.0).round() as i64).clamp(0, 1_999)
}

/// 限流作用域：即时赌局与农场**分开计数**。
/// 农场是慢玩法，一次种满 6 块地不该吃掉 6 次即时下注额度（旧实现共用 `rl:games`）。
#[derive(Clone, Copy)]
pub(super) enum RateScope {
    Instant,
    Farm,
}

impl RateScope {
    fn redis_key(&self, user_id: i64) -> String {
        match self {
            RateScope::Instant => format!("rl:games:{user_id}"),
            RateScope::Farm => format!("rl:farm:{user_id}"),
        }
    }
    fn setting(&self) -> (&'static str, i64) {
        match self {
            RateScope::Instant => {
                ("games_max_plays_per_hour", MAX_PLAYS_PER_HOUR)
            }
            RateScope::Farm => ("farm_max_plays_per_hour", 30),
        }
    }
}

/// 每小时限次（Redis INCR + EXPIRE；上限走设置键）。
/// 审计修复（P2-7）：Redis 不可用时**拒绝**（fail-close）——旧实现 `unwrap_or(0)` 会让限流静默失效。
pub(super) async fn check_rate_scoped(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
    scope: RateScope,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let (setting, default) = scope.setting();
    let limit = eco_i64(state, setting, default).await;
    let key = scope.redis_key(user_id);
    let mut conn = redis.clone();
    let n: i64 = conn.incr(&key, 1).await.map_err(|_| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
    })?;
    if n == 1 {
        let _: () = conn.expire(&key, 3600).await.map_err(|_| {
            DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
        })?;
    }
    if n > limit {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

pub(super) async fn check_rate(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
) -> DomainResult<()> {
    check_rate_scoped(state, redis, user_id, RateScope::Instant).await
}

/// 读取限流器当前已用次数（只读，不 +1；供前端显示「剩余 N 局」）。
/// 取不到（Redis 未计数/异常）返回 None，调用方自行回落。
pub(super) async fn limit_used(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    farm: bool,
) -> Option<i64> {
    use redis::AsyncCommands;
    let key = if farm {
        RateScope::Farm.redis_key(user_id)
    } else {
        RateScope::Instant.redis_key(user_id)
    };
    let mut conn = state.redis.clone();
    conn.get::<_, Option<i64>>(&key).await.ok().flatten()
}

/// 下注校验（上限走设置键 games_max_bet）
pub(super) async fn check_bet(
    state: &web::Data<std::sync::Arc<AppState>>,
    bet: i64,
) -> Result<(), String> {
    let max = eco_i64(state, "games_max_bet", games::MAX_BET).await;
    if bet <= 0 {
        return Err("下注必须为正数".into());
    }
    if bet > max {
        return Err(format!("单次下注不能超过 {max} 魔力"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub(super) struct BetReq {
    pub(super) bet: i64,
    /// 客户端幂等键（审计 P2-8）：网络重试/双击须带同一键，缺省回落随机键保持兼容
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

pub(super) fn idem_key(
    prefix: &str,
    user_id: i64,
    client: &Option<String>,
) -> String {
    match client {
        Some(k) if !k.trim().is_empty() && k.len() <= 128 => {
            format!("game-{prefix}:{user_id}:{k}")
        }
        _ => format!("game-{prefix}:{}:{}", user_id, Uuid::new_v4()),
    }
}

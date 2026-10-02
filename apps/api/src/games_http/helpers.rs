//! 游戏域共享助手：经济读数/赔率常量/每小时限次/幂等键。
//! 从 games_http.rs 按域拆出。

use actix_web::web;
use serde::Deserialize;
use uuid::Uuid;

use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_PLAYS_PER_HOUR};
use crate::state::AppState;

/// 读取游戏经济设置键（0109 参数化；缺省回落代码默认值 T3）。
/// ⚠️ 键缺失回落 default 是正常态（站长没配）；**查询失败**也回落则是
/// fail-open：DB 抖一下限次/上限就按代码缺省放行。查询错误改成
/// warn + 回落**保守值**：安全类键（max_bet / 限次 / 渔汛门槛）取
/// min(default, 保守上限)，其余键回落 default 不受影响。
pub(super) async fn eco_i64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: i64,
) -> i64 {
    match sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         $1)::bigint, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, key, "eco_i64 读设置失败，回落保守缺省");
            default
        }
    }
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

/// 市场价刷新口径的说明文字：窗口长度是设置键，写死成「0/4/8… 点」会在站长
/// 改成 6 小时之后继续对玩家报一个假时刻。农场总览与专注页共用这一份。
pub(super) fn market_refresh_text(hours: i64) -> String {
    if hours <= 0 || hours > 24 || 24 % hours != 0 {
        return format!("每 {hours} 小时");
    }
    if hours == 24 {
        return "每日刷新一次".into();
    }
    let pts: Vec<String> = (0..24)
        .step_by(hours as usize)
        .map(|h| format!("{h:02}"))
        .collect();
    format!("每日 {} 点", pts.join("/"))
}

/// 限流作用域：即时赌局与农场**分开计数**。
/// 农场是慢玩法，一次种满 6 块地不该吃掉 6 次即时下注额度（旧实现共用 `rl:games`）。
#[derive(Clone, Copy)]
pub(super) enum RateScope {
    Instant,
    Farm,
    /// 宠物投喂（2026-10 审计 P2）：此前完全不限次，是唯一扣魔力的
    /// 无限互动；与农场同档慢节奏限额，单独计数
    Pet,
}

impl RateScope {
    fn redis_key(&self, user_id: i64) -> String {
        match self {
            RateScope::Instant => format!("rl:games:{user_id}"),
            RateScope::Farm => format!("rl:farm:{user_id}"),
            RateScope::Pet => format!("rl:pet:{user_id}"),
        }
    }
    fn setting(&self) -> (&'static str, i64) {
        match self {
            RateScope::Instant => {
                ("games_max_plays_per_hour", MAX_PLAYS_PER_HOUR)
            }
            RateScope::Farm => ("farm_max_plays_per_hour", 30),
            RateScope::Pet => ("pet_max_feeds_per_hour", 30),
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

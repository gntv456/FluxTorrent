//! 限流与计数设施（P-61 从 auth_infra.rs 拆出，P-65/P-67 收口）。
//!
//! 这里曾经埋过两个方向的坑，都出过事：
//! - **一档通用**（P-61）：`throttle` 只有登录档 5 次/分，却被十个出口共用，
//!   附件读取/种子下载/档案页注释里写的 60/20/60 一个都没落地；
//! - **fail-open**（P-67）：`incr(...).unwrap_or(0)` 把「Redis 拿不到计数」
//!   当成「还没超限」——Redis 一抖动，限流与账号锁定整条静默失守。
//!   ZT81 已把 `throttle` 收成 fail-close，本模块把剩下的自写计数一并收口，
//!   并规定：**读计数失败就是错误，不是零**。

use std::sync::Arc;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 登录/敏感操作档（60s 固定窗口）：login/register/apply/email-verify/
/// sub-link 用这一档，超限即 429。
const LOGIN_THROTTLE_MAX: i64 = 5;

/// Redis 故障统一 fail-close（ZT81 口径）：`unwrap_or(0)` 会让计数恒 0
/// ⇒ 限流永不触发，抖动期间爆破/撞库/凭据重放全部失守。
fn rl_err(e: redis::RedisError) -> DomainError {
    DomainError::Internal(anyhow::anyhow!("限流服务不可用: {e}"))
}

/// 通用限流（缺省按登录档 5 次/分）。
///
/// 天然频率不同的出口**不要**用这一档，改用 `throttle_limited` 显式带上
/// 自己注释里的数字（P-61：一页第 6 张缩略图 429、第 6 次收种 429、
/// 连看第 6 个主页 429，都是共用登录档造成的）。
pub async fn throttle(state: &Arc<AppState>, key: String) -> DomainResult<()> {
    throttle_limited(state, key, LOGIN_THROTTLE_MAX).await
}

/// 带显式上限的 60s 窗口限流。
pub async fn throttle_limited(
    state: &Arc<AppState>,
    key: String,
    max: i64,
) -> DomainResult<()> {
    rate_window(state, &key, max, 60).await
}

/// 固定窗口计数。**窗口写在键名里**（`rl:<key>:<窗口序号>`），TTL 只为
/// 回收存在——这样滑动续期不会影响窗口复位（P-65：我第一版改成每次
/// EXPIRE，结果闸门一边退避重试一边给自己把 429 续命）。
/// 跨窗口最坏 2×max 的尖峰是所有固定窗口的通性，不额外处理。
pub async fn rate_window(
    state: &Arc<AppState>,
    key: &str,
    max: i64,
    window: i64,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let window = window.max(1);
    let bucket = chrono::Utc::now().timestamp() / window;
    let k = format!("rl:{key}:{bucket}");
    let mut c = state.redis.clone();
    let n: i64 = c.incr(&k, 1).await.map_err(rl_err)?;
    // 留两窗：当前桶够用，旧桶自然回收
    let touched: redis::RedisResult<()> = c.expire(&k, window * 2).await;
    if let Err(e) = touched {
        tracing::warn!(%e, key = %k, "限流桶续期失败（只影响回收，不影响窗口）");
    }
    if n > max {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

/// 读一个计数键（fail-close）。
///
/// 用在「读侧决定放行」的地方（登录账号锁定）：`unwrap_or(0)` 等于
/// 「Redis 挂了就当没锁」，锁定防线整条失守——这是本模块最坏的一处。
pub async fn counter_peek(
    state: &Arc<AppState>,
    key: &str,
) -> DomainResult<i64> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    c.get(key).await.map_err(rl_err)
}

/// 计数 +1 并把 TTL 续到 `ttl` 秒（fail-close）。
///
/// 与 `rate_window` 的区别是**语义**：acctlock 这类「连续失败计数」要的是
/// 「距最后一次失败起算 N 秒」，每次续期才是正确窗口，所以不进键名。
pub async fn counter_bump(
    state: &Arc<AppState>,
    key: &str,
    ttl: i64,
) -> DomainResult<i64> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let n: i64 = c.incr(key, 1).await.map_err(rl_err)?;
    let touched: redis::RedisResult<()> = c.expire(key, ttl).await;
    if let Err(e) = touched {
        tracing::warn!(%e, key = %key, "计数键续期失败（下一次调用会补上）");
    }
    Ok(n)
}

/// 清计数；失败只记 warn。调用点几乎都在「已成功」的路径上
/// （登录成功清锁定）——为一个清理动作把成功的请求打成 5xx 更糟。
pub async fn counter_clear(state: &Arc<AppState>, key: &str) {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let touched: redis::RedisResult<()> = c.del(key).await;
    if let Err(e) = touched {
        tracing::warn!(%e, key = %key, "计数键清理失败（会按 TTL 自然过期）");
    }
}

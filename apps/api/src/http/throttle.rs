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
/// P-423：带**键名**的故障口径。现网 66 条 `限流服务不可用`
/// （2026-10-12~10-19，另有 33 条 RSS 侧同事件）只留下
/// `ResponseError: value is not an integer or out of range` 与
/// `TypeError: Could not convert`，两种都指向「计数器键撞上了字符串值」，
/// 但日志里**没有键名**，事后 Redis TTL 已过、值无从再看 ⇒ 无法归因。
/// 旧版 `rl_err`（不带键）已删除：三个计数器出口都拿得到键。
fn rl_err_k(e: redis::RedisError, key: &str) -> DomainError {
    DomainError::Internal(anyhow::anyhow!(
        "限流服务不可用 [key={key}]: {e}"
    ))
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
    rate_window(state, &key, max, 60).await.map(|_| ())
}

/// 固定窗口计数，**返回本窗口内是第几次调用**（开放 API 要往响应头报
/// 已用/剩余，靠这个返回值，不再自己 INCR）。
/// 窗口写在键名里（`rl:<key>:<窗口序号>`），TTL 只为回收存在——这样滑动
/// 续期不会影响窗口复位（P-65：我第一版改成每次 EXPIRE，结果闸门一边
/// 退避重试一边给自己把 429 续命）。
/// 跨窗口最坏 2×max 的尖峰是所有固定窗口的通性，不额外处理。
///
/// **键名契约（P-72）**：`rl:` 前缀由这里统一加，调用方只传业务段
/// （`npdl:<hash16>`、`att:<uid>`）。七处调用点原本自带 `rl:`，于是线上
/// 真实键是 `rl:rl:npdl:…`（实测抓到的就是这个形状）——闸门脚本按文档里的
/// `rl:npdl:…` 投毒打不中，「fail-close 生效」这类判据会假绿；运维照文档
/// 写的 `redis-cli --scan --pattern` 同样一条都数不到。
pub async fn rate_window(
    state: &Arc<AppState>,
    key: &str,
    max: i64,
    window: i64,
) -> DomainResult<i64> {
    use redis::AsyncCommands;
    let window = window.max(1);
    let bucket = chrono::Utc::now().timestamp() / window;
    let k = format!("rl:{key}:{bucket}");
    let mut c = state.redis.clone();
    let n: i64 = c
        .incr(&k, 1)
        .await
        .map_err(|e| rl_err_k(e, &k))?;
    // 留两窗：当前桶够用，旧桶自然回收
    let touched: redis::RedisResult<()> = c.expire(&k, window * 2).await;
    if let Err(e) = touched {
        tracing::warn!(%e, key = %k, "限流桶续期失败（只影响回收，不影响窗口）");
    }
    if n > max {
        return Err(DomainError::RateLimited);
    }
    Ok(n)
}

/// 读一个计数键。
///
/// 用在「读侧决定放行」的地方（登录账号锁定）：`unwrap_or(0)` 等于
/// 「Redis 挂了就当没锁」，锁定防线整条失守——这是本模块最坏的一处。
///
/// 但 fail-close 不能写歪：键**不存在**时 Redis 的 GET 返回 nil，而
/// `get::<_, i64>` 把 nil 判成类型错误 ⇒ 每个「从没失败过」的账号都 500，
/// 我第一版就把登录打成了全量不可用（闸门在 `login()` 第一步当场撞死）。
/// 正确形状是 `Option<i64>`：没键 = 0，命令真报错才 fail-close。
pub async fn counter_peek(
    state: &Arc<AppState>,
    key: &str,
) -> DomainResult<i64> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let v: Option<i64> = c
        .get(key)
        .await
        .map_err(|e| rl_err_k(e, key))?;
    Ok(v.unwrap_or(0))
}

/// 计数 +1 并把 TTL 续到 `ttl` 秒（fail-close）。
///
/// 与 `rate_window` 的区别是**语义**：acctlock 这类「连续失败计数」要的是
/// 「距最后一次失败起算 N 秒」，每次续期才是正确窗口，所以不进键名。
///
/// 别拿它做限流（P-71，我在 P-67 里差点带出去）：在限流桶上每次续期等于把
/// 固定窗写成**空闲超时窗**——只要客户端持续调用，计数就永不复位，
/// 慢速但不停的正常客户端会被永久锁死。
pub async fn counter_bump(
    state: &Arc<AppState>,
    key: &str,
    ttl: i64,
) -> DomainResult<i64> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let n: i64 = c
        .incr(key, 1)
        .await
        .map_err(|e| rl_err_k(e, key))?;
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

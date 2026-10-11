//! 60s 固定窗口限流（从 auth_infra.rs 拆出，P-61）。
//!
//! 拆出来是因为这里曾经埋过一个口径塌缩：`throttle` 只有一档
//! `LOGIN_THROTTLE_MAX = 5`，却被十个出口共用——附件读取/种子下载/档案页
//! 的注释各写着 60、20、60 次/分，实际全都跑在 5 次/分上。

use std::sync::Arc;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 登录/敏感操作档（60s 固定窗口）：login/register/apply/email-verify/
/// sub-link 用这一档，超限即 429。
const LOGIN_THROTTLE_MAX: i64 = 5;

/// 通用限流（缺省按登录档 5 次/分）。
///
/// P-61（批二十九实测）：天然频率不同的出口**不要**再用这一档——
/// 一页第 6 张缩略图 429（裂图）、第 6 次收种点击 429、连看第 6 个主页
/// 429，都是共用登录档造成的；这类出口改用 `throttle_limited` 显式带上
/// 自己注释里写的数字。
pub async fn throttle(state: &Arc<AppState>, key: String) -> DomainResult<()> {
    throttle_limited(state, key, LOGIN_THROTTLE_MAX).await
}

/// 带显式上限的限流。**窗口按分钟分桶**（键名里带桶号，与
/// `compat_http::limit_passkey` 的 bucket 写法同惯例）。
pub async fn throttle_limited(
    state: &Arc<AppState>,
    key: String,
    max: i64,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    // ZT81：Redis 故障时 fail-close（原 `unwrap_or(0)` 让计数恒 0 → 限流永不触发，
    // 抖动期间密码爆破/撞库全部失守）。与游戏侧 check_rate_scoped 口径对齐。
    //
    // P-65（本批自己踩出来的）：两种「顺手写法」都有毛病——
    // ① 只在 n==1 时 EXPIRE 且吞掉错误：那条命令一失败就留下无 TTL 的计数键
    //    ⇒ 这个用户在这个面上**永久 429**（前一轮审计记过这一手）；
    // ② 每次调用都 EXPIRE（我在 aa1011r 上的第一版修法）：窗口跟着流量滑动，
    //    闸门一边退避重试一边把 429 续命，等 75 秒再跑仍然进不来。
    // 分桶把两件事拆开：窗口在**键名**里（换分钟即重开，与 TTL 无关），
    // TTL 只为回收存在，滑动续期无害。跨桶最坏 2×max 的尖峰与所有固定窗口
    // 实现同形，不额外处理。
    let minute = chrono::Utc::now().timestamp() / 60;
    let k = format!("rl:{}:{}", key, minute);
    let n: i64 = c
        .incr(&k, 1)
        .await
        .map_err(|e| DomainError::Internal(anyhow::anyhow!("限流服务不可用: {e}")))?;
    // TTL 给 2 分钟：当前桶留够，上一桶自然被清
    let touched: redis::RedisResult<()> = c.expire(&k, 120).await;
    if let Err(e) = touched {
        tracing::warn!(%e, key = %k, "限流桶续期失败（只影响回收，不影响窗口）");
    }
    if n > max {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

//! 跨进程配置失效统一通道（0224 G30，吸收 G22 机制部分）。
//!
//! 现状缺口（附录 D 档二）：术语快照（terms.rs）、模块/插件开关缓存
//! （modules.rs / plugins.rs）都是进程内状态，写侧只失效**本进程**——
//! 多副本下 A 改了 B 不知道（术语永不跟随直到重启），单实例下 30s TTL
//! 兜底才让「设置已保存」看起来生效。settings:changed 的 publish 全仓
//! 零订阅者，属否定式假通过（G30 取证教训）。
//!
//! 设计：沿用 flux:guard:ver 已验证的「版本键 + 3s 轮询」范式（不引
//! pub/sub：消息不丢、断线自愈、失败静默降级回 TTL 行为）：
//! - 写侧：`bump("terms")` / `bump("modules")` → HINCRBY flux:cfg:ver；
//! - 订阅侧：api 启动时 spawn 每 3s HGETALL，与本地版本比对，命中域才
//!   执行对应 reload/invalidate；
//! - 降级：Redis 不可达时轮询失败静默（warn 一次/分钟量级），行为退回
//!   现状（术语停旧值、开关 ≤30s TTL）——只增不减。

use redis::AsyncCommands;

/// 写侧：域版本 +1（所有 api 副本的订阅轮询会在 ≤3s 内看到）。
pub async fn bump(state: &crate::state::AppState, domain: &str) {
    let mut c = state.redis.clone();
    let _: Result<i64, _> = c.hincr("flux:cfg:ver", domain, 1).await;
}

/// 订阅侧：每 3s 比对版本，命中域执行对应本地失效。
/// 启动时调一次（main.rs），自身不再退出；redis 断连由 ConnectionManager 自愈。
pub fn spawn_poll(state: std::sync::Arc<crate::state::AppState>) {
    tokio::spawn(async move {
        let mut last: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tick.tick().await;
            let mut c = state.redis.clone();
            let vers: std::collections::HashMap<String, i64> = match c
                .hgetall("flux:cfg:ver")
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    // 降级：退回 TTL 行为，不刷屏
                    tracing::debug!(?e, "cfg:ver 轮询失败（降级为 TTL 兜底）");
                    continue;
                }
            };
            for (domain, v) in vers {
                let changed = match last.get(&domain) {
                    Some(prev) => *prev != v,
                    None => false, // 首见不触发（启动时各域已从库装载最新）
                };
                if changed {
                    tracing::info!(domain, "配置变更跨进程生效");
                    match domain.as_str() {
                        "terms" => {
                            crate::terms::reload(&state.repo.db).await;
                        }
                        "modules" => {
                            state.module_flags.invalidate().await;
                            crate::plugins::invalidate_plugin_cache();
                        }
                        _ => {}
                    }
                }
                last.insert(domain, v);
            }
        }
    });
}

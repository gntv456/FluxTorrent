//! 消费者组例清（审计 10-07 P2-3，从 group.rs 拆出，300 行门禁）。
//!
//! 只有一件事：把「进程早没了、名字还挂在消费者组里」的旧消费者删掉，
//! 判据与安全边界见 `reap_dead_consumers`。

use redis::aio::ConnectionManager;

use super::group::GroupCfg;

/// 回收死消费者：`consumer_name()` = `tag-pid` ⇒ 每次重启都沉积一个死消费者
/// （实测 `XINFO GROUPS` 显示 consumers=24 而进程只有 1 个），PEL 归属随之变
/// 孤儿、面板失真。只删 idle>1h **且 pending=0** 的——带 pending 的绝不删（那正是
/// XAUTOCLAIM 要接手的遗孤）；10min 节流，避免每轮消费多付两次 RTT。
pub(crate) async fn reap_dead_consumers(
    redis: &mut ConnectionManager,
    cfg: &GroupCfg,
) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(LAST.load(Ordering::Relaxed)) < 600 {
        return;
    }
    LAST.store(now, Ordering::Relaxed);
    let reply: redis::Value = redis::cmd("XINFO")
        .arg("CONSUMERS")
        .arg(&cfg.stream)
        .arg(&cfg.group)
        .query_async(redis)
        .await
        .unwrap_or(redis::Value::Nil);
    let entries = match reply {
        redis::Value::Array(v) => v,
        _ => return,
    };
    let mut doomed: Vec<String> = Vec::new();
    for entry in entries {
        let fields = match entry {
            redis::Value::Array(f) => f,
            _ => continue,
        };
        let mut name = String::new();
        let mut pending = i64::MAX;
        let mut idle_ms = u64::MAX;
        let mut it = fields.into_iter();
        while let (Some(k), Some(v)) = (it.next(), it.next()) {
            let key =
                redis::from_owned_redis_value::<String>(k).unwrap_or_default();
            match key.as_str() {
                "name" => {
                    name = redis::from_owned_redis_value::<String>(v)
                        .unwrap_or_default();
                }
                "pending" => {
                    pending = redis::from_owned_redis_value::<i64>(v)
                        .unwrap_or(i64::MAX);
                }
                "idle" => {
                    idle_ms = redis::from_owned_redis_value::<u64>(v)
                        .unwrap_or(u64::MAX);
                }
                _ => {}
            }
        }
        if !name.is_empty() && pending == 0 && idle_ms > 3_600_000 {
            doomed.push(name);
        }
    }
    for name in doomed {
        let r: Result<i64, redis::RedisError> = redis::cmd("XGROUP")
            .arg("DELCONSUMER")
            .arg(&cfg.stream)
            .arg(&cfg.group)
            .arg(&name)
            .query_async(redis)
            .await;
        if r.is_ok() {
            tracing::info!(consumer = %name, stream = cfg.stream, "回收空闲消费者");
        }
    }
}

//! Stream 消费者组共享设施（0225 G30-B2）。
//!
//! 现状缺口（附录 D 档三）：consume_announce / consume_agent_blocks 用
//! 「全局游标 + XRANGE」——天生「唯一执行者」假设，两实例并发拉同一段流
//! 会双计费（此前靠 advisory 锁把第二台锁成旁观者 = 热备不是扩展）。
//!
//! 改造：XREADGROUP 消费者组（组内消息只投给一个消费者，多 worker 真分摊）
//! + XAUTOCLAIM 回收遗孤 pending（worker 崩溃后由存活实例接手）。
//!
//! 语义保持：处理失败不 ACK 留 PEL 重试；连续 6 次进 DLQ 后 ACK；
//! 旧全局游标一次性迁移（组从游标位置创建，键删除退役）。
//!
//! 形态：本模块只提供「取原始条目 + ACK 原语」，事件处理循环留在各
//! consume_*（与原 XRANGE 循环体一一对应，不动业务语义）。

use redis::aio::ConnectionManager;

/// 消费者组参数（两条流各一份常量）。
pub(crate) struct GroupCfg {
    pub stream: &'static str,
    pub group: &'static str,
    /// 兼容迁移用的旧全局游标键
    pub legacy_cursor: &'static str,
    /// DLQ list 键
    pub dlq: &'static str,
    /// 单轮最多拉取条数
    pub batch: usize,
}

pub(crate) const ANNOUNCE_GROUP: GroupCfg = GroupCfg {
    stream: "flux:announce",
    group: "fluxcg-announce",
    legacy_cursor: "flux:announce:cursor",
    dlq: "flux:announce:dlq",
    batch: 200,
};

pub(crate) const AGENTBLOCK_GROUP: GroupCfg = GroupCfg {
    stream: "flux:agent_block",
    group: "fluxcg-agentblock",
    legacy_cursor: "flux:agentblock:cursor",
    dlq: "flux:agentblock:dlq",
    batch: 100,
};

impl GroupCfg {
    /// 消息成功（或安全跳过）后 ACK——不 ACK 即留 PEL 待重试
    pub(crate) async fn ack(&self, redis: &mut ConnectionManager, id: &str) {
        let _: Result<(), _> = redis::cmd("XACK")
            .arg(self.stream)
            .arg(self.group)
            .arg(id)
            .query_async(redis)
            .await;
    }

    pub(crate) async fn dlq_push_with(
        &self,
        redis: &mut ConnectionManager,
        item: &str,
    ) {
        let _: Result<(), _> = redis::cmd("RPUSH")
            .arg(self.dlq)
            .arg(item)
            .query_async(redis)
            .await;
    }
}

/// 消费者名：实例标识 + PID（同机多进程也不重名；重启即新消费者，
/// 旧消费者的 pending 由 XAUTOCLAIM 回收）。
pub(crate) fn consumer_name() -> String {
    let tag = crate::shutdown::instance_tag();
    let pid = std::process::id();
    if tag.is_empty() {
        format!("w-{pid}")
    } else {
        format!("{tag}-{pid}")
    }
}

/// 一次性迁移：旧全局游标 → 消费者组起点（组从游标 ID 创建，老机制退役）。
/// 幂等：组已存在直接返回；并发创建竞争（BUSYGROUP）静默。
async fn ensure_group(redis: &mut ConnectionManager, cfg: &GroupCfg) {
    use redis::AsyncCommands;
    let info: Result<redis::Value, _> = redis::cmd("XINFO")
        .arg("GROUPS")
        .arg(cfg.stream)
        .query_async(redis)
        .await;
    let exists = info
        .map(|v| format!("{v:?}").contains(cfg.group))
        .unwrap_or(false);
    if exists {
        return;
    }
    // 起点：旧游标（有则从它之后继续）否则 $（只消费新事件——历史事件在
    // 单实例时代已被游标消费过；空库首启无历史，$ 与 0 等价）
    // 审查修正（P0-4）：游标读取失败若静默回落 "$" 会跳过旧游标前未处理
    // 完的事件——读失败必须 fail-hard（本轮放弃建组，下轮重试）；
    // 只有「键不存在」才允许 $ 起点。
    let last: Option<String> = match redis
        .get::<_, Option<String>>(cfg.legacy_cursor)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(?e, stream = cfg.stream,
                "旧游标读取失败，延迟建组（避免 $ 起点丢事件）");
            return;
        }
    };
    let start = last.clone().unwrap_or_else(|| "$".to_string());
    let created: Result<(), redis::RedisError> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(cfg.stream)
        .arg(cfg.group)
        .arg(&start)
        .arg("MKSTREAM")
        .query_async(redis)
        .await;
    match created {
        Ok(()) => {
            tracing::info!(
                stream = cfg.stream,
                group = cfg.group,
                "消费者组已创建（起点 {start}，旧游标机制退役）"
            );
            let _: Result<i64, _> = redis.del(cfg.legacy_cursor).await;
        }
        Err(e) => {
            if !format!("{e}").contains("BUSYGROUP") {
                tracing::warn!(
                    ?e,
                    stream = cfg.stream,
                    "消费者组创建失败（下轮重试）"
                );
            }
        }
    }
}

/// 拉取新消息（">"）：返回 (id, payload) 原始条目，处理与 ACK 留给调用方。
pub(crate) async fn read_group(
    redis: &mut ConnectionManager,
    cfg: &GroupCfg,
    consumer: &str,
) -> Vec<(String, String)> {
    ensure_group(redis, cfg).await;
    let reply = redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg(cfg.group)
        .arg(consumer)
        .arg("COUNT")
        .arg(cfg.batch)
        .arg("STREAMS")
        .arg(cfg.stream)
        .arg(">")
        .query_async::<redis::Value>(redis)
        .await;
    match reply {
        Ok(v) => parse_stream_reply(&v),
        Err(e) => {
            tracing::error!(?e, stream = cfg.stream, "XREADGROUP 失败");
            Vec::new()
        }
    }
}

/// 回收遗孤 pending（空闲 > min_idle_ms 的消息接管重试一轮）。
/// 返回 (id, payload)；处理结果同样由调用方决定 ACK 与否。
pub(crate) async fn reclaim_stale(
    redis: &mut ConnectionManager,
    cfg: &GroupCfg,
    consumer: &str,
    min_idle_ms: usize,
) -> Vec<(String, String)> {
    let reply = redis::cmd("XAUTOCLAIM")
        .arg(cfg.stream)
        .arg(cfg.group)
        .arg(consumer)
        .arg(min_idle_ms)
        .arg("0-0")
        .arg("COUNT")
        .arg(cfg.batch)
        .query_async::<redis::Value>(redis)
        .await;
    match reply {
        Ok(v) => parse_autoclaim_reply(&v),
        Err(e) => {
            // NOGROUP（组还没建）静默；其余仅告警不阻塞
            if !format!("{e}").contains("NOGROUP") {
                tracing::warn!(?e, stream = cfg.stream, "XAUTOCLAIM 失败");
            }
            Vec::new()
        }
    }
}

/// 解析 XREADGROUP 的 redis::Value（nil / [[stream, entries]..]）。
fn parse_stream_reply(v: &redis::Value) -> Vec<(String, String)> {
    match v {
        redis::Value::Array(items) => items
            .iter()
            .filter_map(|se| match se {
                redis::Value::Array(pair) if pair.len() >= 2 => {
                    parse_entries(&pair[1])
                }
                _ => None,
            })
            .flatten()
            .collect(),
        _ => Vec::new(),
    }
}

/// 解析 XAUTOCLAIM 回复——entries 都在下标 1：Redis 7 为
/// [next-id, entries, deleted] 三元，6.2 为 [next-id, entries] 二元
/// （两种形态兼容；当前 compose 固定 redis:7）。
fn parse_autoclaim_reply(v: &redis::Value) -> Vec<(String, String)> {
    match v {
        redis::Value::Array(items) if items.len() >= 2 => {
            parse_entries(&items[1]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// 条目列表 → (id, payload)；无 payload 字段的损坏条目返回 ("", "") 由
/// 调用方按 DLQ 处理。
fn parse_entries(v: &redis::Value) -> Option<Vec<(String, String)>> {
    let redis::Value::Array(entries) = v else {
        return None;
    };
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let redis::Value::Array(pair) = e else {
            continue;
        };
        if pair.len() < 2 {
            continue;
        }
        let id = value_to_string(&pair[0]);
        let payload = flat_fields(&pair[1])
            .into_iter()
            .find(|(k, _)| k == "payload")
            .map(|(_, v)| v)
            .unwrap_or_default();
        out.push((id, payload));
    }
    Some(out)
}

/// XID 在 redis 0.27 里可能是 BulkString("ms-seq") 或 Bulk([ms, seq])
fn value_to_string(v: &redis::Value) -> String {
    match v {
        redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
        redis::Value::Array(parts) => parts
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("-"),
        other => format!("{other:?}"),
    }
}

/// field-value 扁平列表 → (k, v) 对
fn flat_fields(v: &redis::Value) -> Vec<(String, String)> {
    let redis::Value::Array(items) = v else {
        return Vec::new();
    };
    items
        .chunks(2)
        .filter_map(|c| match (&c[0], c.get(1)) {
            (
                redis::Value::BulkString(k),
                Some(redis::Value::BulkString(val)),
            ) => Some((
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(val).to_string(),
            )),
            _ => None,
        })
        .collect()
}

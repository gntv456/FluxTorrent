//! announce 主消费 + process_event 计费。
//! 从 jobs.rs 按域拆出。
//!
//! 0225 G30-B2：全局游标 XRANGE → 消费者组 XREADGROUP（组内投递唯一，
//! 多 worker 真分摊）；失败语义不变——不 ACK 留 PEL 重试，连续 6 次进
//! DLQ 后 ACK；旧游标兼容迁移见 group.rs::ensure_group。

use super::announce_handle::handle_entry;
use super::group::{self, ANNOUNCE_GROUP};
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 点刷在途标志：保证同一时刻只有一个计数点刷任务在跑（下一轮覆盖即可）。
static FLUSH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
/// 上次点刷的 Unix 时刻（每实例 5 分钟最多一次）。
static LAST_FLUSH_AT: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(0);
/// 全局点刷闸（Redis SETNX，60s TTL）——同一时刻只允许一个实例在点刷。
const FLUSH_LOCK_KEY: &str = "flux:announce:flush-lock";
/// 点刷的最小间隔（秒）：每实例 5 分钟最多刷一次。
const FLUSH_MIN_INTERVAL_SECS: i64 = 300;

/// 单实例内 announce 事件的并发处理上限。
///
/// ⚠️ 必须有界（2026-11-06 实测事故）：旧写法把本轮 5,000 条事件
/// **一次性全部 spawn**，5,000 个任务同时向 32 条池的连接池要连接，
/// 排队时间超过 sqlx 默认 30s acquire timeout ⇒ 批量报
/// "pool timed out while waiting for an open connection" ⇒ 事件不 ACK
/// 留在 PEL ⇒ 下一轮重投再失败，活锁：lag 不降、pending 不消。
/// 事件本身幂等（announce_seen 键），有界并发只是把排队从**连接池**
/// 挪到**信号量**——池永远拿得到连接，等待不会超时。
///
/// ⚠️ 上限 12 而非 16（2026-11-06 第二次事故）：3 个消费实例 × 16 = 48 个
/// 在途事务把 PG 顶到 336% CPU，PG 容器（限额 1 GB）的 cgroup 被 OOM
/// killer 打断——一个 postgres 后端被杀 ⇒ PG 进入恢复 ⇒ 全站短暂拒连。
/// 单后端的 work_mem（4 MB/排序节点）+ 共享内存映射 × 并发数才是吃满
/// cgroup 的真凶。限额已抬到 2 GB（compose 已改），并发取 12 × 3 = 36：
/// 实测每事件约 10~15 次 DB 往返，吞吐 ~5,000 条/分（生产 ~4,300 条/分），
/// 12 能把消费拉到 ~7,500 条/分，积压回追有余量。
const ANNOUNCE_CONCURRENCY: usize = 12;

/// 消费 announce 事件流（§5.4 链路 ④-⑦）：Redis Stream → 计费流水 + snatches。
/// 事件为 JSON 文本，字段 user/hash/up/down/event/left。
pub async fn consume_announce(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    // 保种时长累计容忍窗 = 2 × announce_interval（与 tracker 下发口径一致，站点设定缺省 1800）。
    // 客户端按 interval 汇报，相邻两次做种 announce 的时间差即真实做种时长；
    // 超窗（离线/故障）不计，防挂机伪造做种时长。
    let announce_interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let seed_cap = (announce_interval * 2).clamp(3600, 172_800);

    let consumer = group::consumer_name();
    let mut touched_users: BTreeSet<i64> = BTreeSet::new();
    let mut touched_torrents: BTreeSet<i64> = BTreeSet::new();
    let mut applied = 0u64;

    // 新消息（2026-11-06：改为**有界并发**）。原写法逐条 await，每条事件
    // 一个 DB 事务约 30ms ⇒ 单实例吞吐被锁在 ~2,000 条/分；迁站后全站
    // 13 万活跃 peer 的稳态生产约 4,400 条/分 ⇒ 事件无限积压（lag 一度
    // 13 万），做种数与流量计费全面滞后。每条事件独立事务、幂等键
    // announce_seen 防重入，天然可并发；并发度由信号量封顶（见
    // ANNOUNCE_CONCURRENCY），不靠连接池去隐式限流。
    let entries = group::read_group(redis, &ANNOUNCE_GROUP, &consumer).await;
    let mut tasks = tokio::task::JoinSet::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(ANNOUNCE_CONCURRENCY));
    for (id, payload) in entries {
        if payload.is_empty() {
            ANNOUNCE_GROUP
                .dlq_push_with(redis, &format!("{id}\t(no payload)"))
                .await;
            ANNOUNCE_GROUP.ack(redis, &id).await;
            applied += 1;
            continue;
        }
        let db2 = db.clone();
        let mut r2 = redis.clone();
        let sid = id.clone();
        let sem2 = sem.clone();
        tasks.spawn(async move {
            // 许可在事件处理期间持有（含全部 DB 往返），确保在途事务数
            // 不超上限——否则池会被瞬间抽干，队后面的任务全部超时。
            let _permit = sem2.acquire().await.ok();
            let r = handle_entry(&db2, &mut r2, &sid, &payload, seed_cap).await;
            (sid, r)
        });
    }
    while let Some(res) = tasks.join_next().await {
        let (id, r) = match res {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(?e, "announce 处理任务异常");
                continue;
            }
        };
        match r {
            Ok((ack, u, t)) => {
                if let Some(u) = u {
                    touched_users.insert(u);
                }
                if let Some(t) = t {
                    touched_torrents.insert(t);
                }
                if ack {
                    ANNOUNCE_GROUP.ack(redis, &id).await;
                    applied += 1;
                }
            }
            Err(e) => {
                // 不 ACK：留 PEL 等下一轮重试（连续 6 次由 handle_entry 转死信）
                tracing::error!(?e, %id, "事件处理失败，留 PEL");
            }
        }
    }
    // 回收遗孤 pending（worker 崩溃自愈）：空闲 > 120s
    //
    // ⚠️ 必须并发处理（2026-11-06 实测）：旧写法是**逐条串行** await。
    // 每次容器重启都会给消费者组沉积一批陈旧 PEL（每次 5,000 条量级），
    // 串行处理 5,000 条 × 每次 ~50ms = **单 tick 4 分钟** ⇒ 读事件的
    // 频率被压到 1,000 条/分，低于生产速率 ⇒ 积压永远消不掉。
    // 现与主批次共用同一个信号量（有界并发），单 tick 回到秒级。
    let reclaimed =
        group::reclaim_stale(redis, &ANNOUNCE_GROUP, &consumer, 360_000).await;
    if !reclaimed.is_empty() {
        let mut rtasks = tokio::task::JoinSet::new();
        for (id, payload) in reclaimed {
            if payload.is_empty() {
                continue;
            }
            let db2 = db.clone();
            let mut r2 = redis.clone();
            let sid = id.clone();
            let sem2 = sem.clone();
            rtasks.spawn(async move {
                let _permit = sem2.acquire().await.ok();
                let r =
                    handle_entry(&db2, &mut r2, &sid, &payload, seed_cap).await;
                (sid, r)
            });
        }
        while let Some(res) = rtasks.join_next().await {
            let (id, r) = match res {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(?e, "回收事件处理任务异常");
                    continue;
                }
            };
            match r {
                Ok((ack, u, t)) => {
                    if let Some(u) = u {
                        touched_users.insert(u);
                    }
                    if let Some(t) = t {
                        touched_torrents.insert(t);
                    }
                    if ack {
                        ANNOUNCE_GROUP.ack(redis, &id).await;
                        applied += 1;
                    }
                }
                Err(e) => {
                    // 不 ACK：留 PEL 等下一轮重试（连续 6 次由 handle_entry 转死信）
                    tracing::error!(?e, %id, "回收事件处理失败，留 PEL");
                }
            }
        }
    }
    // XTRIM：以「最旧未确认条目」为界裁，无 pending 才按长度裁（旧写法盲裁
    // 长度，worker 落后时未投递的事件会被削掉）。详见 group::trim_safe。
    group::trim_safe(redis, &ANNOUNCE_GROUP).await;

    // 幂等键表清理（0285）：重投/回收窗口最长 12 分钟，留 3 天已极宽；
    // 就地跟着消费循环裁，不另开 job（避免「表在但没人清」的长跑泄漏）。
    let purged = sqlx::query(
        "DELETE FROM announce_seen WHERE seen_at < now() - interval '3 days'",
    )
    .execute(db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);
    if purged > 20_000 {
        tracing::info!(n = purged, "announce_seen 批量清理");
    }

    // P0-1：快照点刷——仅本轮有事件的用户/种子（权威在流水，快照仅展示）
    if !touched_users.is_empty() {
        let users: Vec<i64> = touched_users.into_iter().collect();
        sqlx::query(
            "UPDATE users SET \
             uploaded = COALESCE((SELECT base_up FROM balance_baseline \
                 WHERE user_id = users.id), 0) \
                 + COALESCE((SELECT sum(delta_up) FROM traffic_ledger \
                 WHERE user_id = users.id), 0), \
             downloaded = COALESCE((SELECT base_down FROM \
                 balance_baseline WHERE user_id = users.id), 0) \
                 + COALESCE((SELECT sum(delta_down) FROM \
                 traffic_ledger WHERE user_id = users.id), 0) \
             WHERE id = ANY($1)",
        )
        .bind(&users)
        .execute(db)
        .await?;
    }
    if !touched_torrents.is_empty() {
        let torrents: Vec<i64> = touched_torrents.into_iter().collect();
        // 2026-11-06：点刷**挪到后台**、**全局单实例**、**5 分钟一次**。
        //
        // 为什么挪走：点刷要走 snatches 的部分索引做 index-only scan，而
        // snatches 是高频 upsert 表 ⇒ 可见性图常脏 ⇒ index-only scan 退化成
        // 回表 ⇒ 实测单批 1,000 个种子要 30 秒（5,000 个要 104 秒）。放在
        // 消费循环里 await 等于**把整个 tick 堵死**：事件读不进来 → 积压
        // 永远消不掉（消费速率 1,667/分 < 生产 4,300/分）。
        //
        // 为什么加闸：三个消费实例各自点刷 = 3 条重聚合并发，把 CPU 抢掉
        // 一半——实测**取消点刷后消费速率从 3,000/分 翻到 6,000/分**，lag
        // 从"反涨"变成净消 3,186/分。⇒ Redis SETNX 保证同一时刻只有一
        // 个实例在刷，且每实例 5 分钟最多刷一次；覆盖缺口由下一轮与
        // 6h 全量对账兜底（计数是展示快照，权威在流水）。
        let now = chrono::Utc::now().timestamp();
        let last = LAST_FLUSH_AT.load(Ordering::Relaxed);
        let due = now - last >= FLUSH_MIN_INTERVAL_SECS;
        if due && !FLUSH_IN_PROGRESS.swap(true, Ordering::Relaxed) {
            // 全局锁：抢不到就让别的实例刷（本轮跳过，不排队）。
            let got: Option<String> = redis::cmd("SET")
                .arg(FLUSH_LOCK_KEY)
                .arg("1")
                .arg("NX")
                .arg("EX")
                .arg(60)
                .query_async(redis)
                .await
                .ok();
            if got.is_some() {
                LAST_FLUSH_AT.store(now, Ordering::Relaxed);
                let db2 = db.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        super::peer_counts::refresh_peer_counts_light(
                            &db2, &torrents,
                        )
                        .await
                    {
                        tracing::error!(?e, "做种计数点刷失败");
                    }
                    FLUSH_IN_PROGRESS.store(false, Ordering::Relaxed);
                });
            } else {
                FLUSH_IN_PROGRESS.store(false, Ordering::Relaxed);
            }
        }
    }
    Ok(applied)
}

#[derive(serde::Deserialize)]
pub(crate) struct AnnounceEvent {
    pub(crate) user: i64,
    pub(crate) hash: String,
    pub(crate) up: i64,
    pub(crate) down: i64,
    #[serde(default)]
    pub(crate) event: String,
    #[serde(default)]
    pub(crate) left: i64,
    /// announce 来源 IP（0071 反作弊：账号 IP 跳变/多 IP 分析）
    #[serde(default)]
    pub(crate) ip: String,
    /// 上报监听端口（2026-10-07 保种组审计 P0-2）：0=客户端未开监听，
    /// 「幽灵做种」判定基础（port=0 不可能提供上传，不得按在种累计）。
    /// 旧事件缺省 0：升级窗口内的旧客户端按保守口径不计在种（真实客户端
    /// 必带端口，缺省 0 影响面≈零）。
    #[serde(default)]
    pub(crate) port: u16,
    /// tracker 主动回连抽样结果（0071）：-1=未测（缺省/旧事件） 0=不可达 1=可达
    #[serde(default)]
    pub(crate) conn: Option<i16>,
    /// tracker 收到 announce 的时点（RFC3339）。促销按「事件时点」而非「消费时点」裁决：
    /// 事件积压（DLQ 重试/worker 停机）时避免免费窗口结束后按原价补计费。
    #[serde(default)]
    pub(crate) ts: Option<chrono::DateTime<chrono::Utc>>,
    /// BT 客户端 UA（0098 下载列表「客户端」列）；截断 200
    #[serde(default)]
    pub(crate) agent: String,
}

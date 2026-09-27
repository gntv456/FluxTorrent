//! FluxTorrent Tracker —— 轻量私有 Tracker（方案 §2.5 自研兜底路线）。
//! 设计（§5.4 announce 链路）：
//!   ① passkey 鉴权：内存缓存（60s TTL）兜底，命中免查 PG，拒绝无效 passkey
//!   ② peer 状态：DashMap 内存表（90s 超时淘汰）
//!   ③ 立即返回 compact 响应（二进制安全），不阻塞计费
//!   ④ 事件 → Redis Stream（XADD），worker 异步消费计费
//! 高频/异常请求防护（announce 路径从 cheap 到 expensive）：
//!   ⓪ ip_bans 封禁：内存缓存（60s 刷新），命中即拒
//!   ⓪' 频率限流：Redis INCR 滑动窗口 —— 每 IP 阈值在 passkey 之前拦截垃圾流量，
//!      每用户阈值在 passkey 之后（Redis 故障 fail-open，不阻断正常 announce）
//!   ①'' agent_rules 黑白名单：内存缓存（60s 刷新），不再逐请求全表扫描
//!   interval / min interval 按 site_settings.announce_interval 下发（BEP3 强制），
//!   告知客户端汇报间隔，从源头抑制客户端高频重发。
//! BEP3 兼容要点：info_hash/peer_id 是任意字节的 percent-encoding ——
//! 绕过 serde Query 反序列化，直接解析原始 query 字节。

//! 按域拆分（300 行门禁）：HTTP announce 链路在 http_track/announce.rs，
//! 助手在 helpers.rs，scrape/metrics/emit 同目录；main 装配留在此。

mod http_track;
mod peers;
mod udp;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use actix_web::{web, App, HttpServer};
use dashmap::DashMap;
use http_track::helpers::{
    GuardCfg, GuardInner, LocalWindows, Metrics, TrackerState,
};
use http_track::*;
use peers::{Peer, PeerTable};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind =
        std::env::var("TRACKER_BIND").unwrap_or_else(|_| "0.0.0.0:7070".into());
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent".into()
    });
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await?;
    let redis = redis::Client::open(redis_url.as_str())?
        .get_connection_manager()
        .await?;
    let cfg = GuardCfg {
        // 保种大户口径：interval 1800s 时，每分钟 announce 数 ≈ 挂种数/30。
        // 1800/min 覆盖 5.4 万挂种；超限只是计费延迟（累计量差值口径，不丢），且账号可挂起兜底。
        user_per_min: env_i64("ANN_RATE_USER_PER_MIN", 1800),
        // 每 IP 兜底（兼容量子洞 NAT / 多做种盒 / 单 IP 一鲸鱼+常量用户）；
        // 无凭据垃圾洪水仍被限在 60r/s/IP，分布式洪水属边缘网关/WAF 职责。
        ip_per_min: env_i64("ANN_RATE_IP_PER_MIN", 3600),
        // 全局应急熔断：0=关闭；遭分布式洪水时临时设置（如 120000 = 2000r/s 总闸）
        global_per_min: env_i64("ANN_RATE_GLOBAL_PER_MIN", 0),
        default_interval: env_i64("ANN_INTERVAL_DEFAULT", 1800)
            .clamp(60, 86400),
    };
    let state = web::Data::new(TrackerState {
        peers: PeerTable::new(),
        redis: redis.clone(),
        db,
        guard: RwLock::new(GuardInner {
            passkeys: HashMap::new(),
            ip_bans: HashMap::new(),
            agent_rules: None, // None 使首个请求必然触发首次加载（见 refresh_guard 的 stale 判定）
            announce_interval: cfg.default_interval,
            refreshed_at: Instant::now(),
        }),
        cfg,
        metrics: Metrics::default(),
        local: LocalWindows {
            inner: DashMap::new(),
        },
        force_refresh: AtomicBool::new(false),
        ver: AtomicI64::new(0),
    });

    // peer 快照预热（0101）：重启后从 Redis 恢复未超时 peer，缩短做种列表空窗。
    // 客户端 30min 内重 announce 本就可自愈——恢复失败仅记日志，绝不阻塞启动。
    {
        use redis::AsyncCommands;
        let mut c = redis.clone();
        match c.get::<_, Option<String>>("flux:tracker:peers").await {
            Ok(Some(raw)) => match serde_json::from_str::<
                Vec<(String, Vec<Peer>)>,
            >(&raw)
            {
                Ok(snap) => {
                    let n = state.peers.restore(snap);
                    tracing::info!(n, "peer table warm-restored from redis");
                }
                Err(e) => {
                    tracing::warn!(%e, "peer snapshot parse failed, skip warm restore")
                }
            },
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(%e, "peer snapshot read failed, skip warm restore")
            }
        }
    }

    // peer 快照周期落盘（60s）：tracker 是 SPOF，快照让重启从「全量重建」变「增量补齐」
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                let snap = st.peers.export();
                match serde_json::to_string(&snap) {
                    Ok(raw) => {
                        let mut c = st.redis.clone();
                        use redis::AsyncCommands;
                        // 30min TTL：tracker 长时间下线后旧快照不再有效
                        if let Err(e) = c
                            .set_ex::<_, _, ()>("flux:tracker:peers", raw, 1800)
                            .await
                        {
                            tracing::warn!(%e, "peer snapshot write failed");
                        }
                    }
                    Err(e) => tracing::warn!(%e, "peer snapshot encode failed"),
                }
            }
        });
    }

    // 管理端变更通知轮询：flux:guard:ver 每 3s 一查（单 GET，可忽略的开销）。
    // API 侧在 ip_bans/agent_rules/挂起/passkey 变更后 INCR 该键 → 立即刷新缓存+清 passkey。
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            use redis::AsyncCommands;
            let mut conn = st.redis.clone();
            let mut tick = tokio::time::interval(Duration::from_secs(3));
            loop {
                tick.tick().await;
                if let Ok(Some(v)) =
                    conn.get::<_, Option<i64>>("flux:guard:ver").await
                {
                    let last = st.ver.swap(v, Ordering::Relaxed);
                    if last != v {
                        st.force_refresh.store(true, Ordering::Relaxed);
                        st.guard_write().passkeys.clear();
                    }
                }
            }
        });
    }

    // tracker 不订阅 flux:cfg:ver：术语/模块开关不在 tracker 进程内缓存（它只
    // 读 ip_bans/agent_rules/passkey 小表，走上方 guard:ver），保持单一通道不重复。

    // connectable 抽样（0071 P1-9，防假保种）：每 5min 抽 50 个 peer 做 TCP 回连（3s 超时），
    // 未测优先、已测轮替复测。结果写回 peer 表 → 随 announce 事件流入 snatches.connectable，
    // 「不可达 + 零上传」的做种不计做种收益并进作弊探测。纯探测不阻断任何响应路径。
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(300));
            loop {
                tick.tick().await;
                for (key, probe_ip, probe_port) in st.peers.sample_probes(50) {
                    let attempt = tokio::time::timeout(
                        Duration::from_secs(3),
                        tokio::net::TcpStream::connect((
                            probe_ip.as_str(),
                            probe_port,
                        )),
                    )
                    .await;
                    let reachable = matches!(attempt, Ok(Ok(_)));
                    st.peers.set_connectable(&key, reachable);
                }
            }
        });
    }

    // UDP tracker（BEP15）：TRACKER_UDP_BIND 未设置（空）则不启用；
    // 默认 6969。与 HTTP announce 共享 state（peer 表/限流/事件流）。
    let udp_bind = std::env::var("TRACKER_UDP_BIND")
        .unwrap_or_else(|_| "0.0.0.0:6969".into());
    if !udp_bind.is_empty() {
        let udp = std::sync::Arc::new(udp::UdpTracker::new(state.clone()));
        let ub = udp_bind.clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = udp.run(&ub).await {
                tracing::error!(%e, %ub, "UDP tracker exited");
            }
        });
    }

    tracing::info!(
        "flux-tracker listening on {bind}（防护：每用户 {}/min，每 IP {}/min，interval {}s）",
        state.cfg.user_per_min,
        state.cfg.ip_per_min,
        state.cfg.default_interval
    );
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(announce)
            .service(scrape)
            .service(metrics)
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}

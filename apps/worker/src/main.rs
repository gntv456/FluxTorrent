//! FluxTorrent Worker：异步任务组（§5.2）。
//! 职责：announce 计费消费、促销到期回收、保种区移出、做种收益结算、签到连签、
//!       银行每日结算、任务达标结算、考核自动派发。

mod balance_guard;
mod bank_jobs;
mod jobs;
mod runtime_log;
mod shutdown;
mod task_jobs;

/// worker 连接池缺省容量（DB_POOL_SIZE 可覆盖）。并发 job 的槽位在
/// `jobs/locks.rs` 取它的一半——一条 job 在 `with_lock` 里占两条连接，
/// 两个数字必须成倍数关系（闸门 pt_audit_worker_job_health 的 J3 钉这一条）。
pub(crate) const JOB_POOL_DEFAULT: u32 = 32;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    // fmt 层保留 stdout 输出；runtime_log 层把 WARN+ 也写进库（后台「运行日志」页）
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(runtime_log::layer("worker"))
        .init();

    // 优雅停机（0224 G30）：SIGTERM/SIGINT → 停止认领 + 排空在跑手动任务（≤60s）。
    // compose 侧 stop_grace_period 须 ≥ 90s（60s 排空 + 余量），否则 docker 仍 SIGKILL。
    shutdown::install_signal_handler();

    // 凭据走 env 强制注入（compose 是 ${DB_PASSWORD:?} 硬失败）；代码里不留
    // 任何带口令的缺省连接串——开源可见的缺省口令等于给暴露 5432 的部署发钥匙。
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://127.0.0.1:5432/fluxtorrent".into());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    // 0225 G30-B14：并发调度后同 tick 最多 10 定时 + 3 手动任务并发抢连接，
    // 固定 5 会 PoolTimedOut（且被 run_guarded 的笼统文案误报成锁冲突）。
    // 缺省提到 16，可经 DB_POOL_SIZE 覆盖。
    //
    // 2026-10-20 再提到 32：16 仍不够。`with_lock` 每条 job 占两条连接
    // （持 advisory lock 的连接 + 业务体自己那条，见 jobs/locks.rs 注释），
    // 而现网一分钟最多同时起跑 15 条 job（job_status 实测），
    // 于是 16 条池只能养活 8 条，剩下的一律
    // 「pool timed out while waiting for an open connection」——
    // 24 小时内 29 次，集中在 hr_punish / hr_warn_ladder / cheat_enforce /
    // collusion_check / class_auto_adjust / sweep_stale_peers 这一族。
    // 32 = 15 条并发 × 2 + 余量；PG max_connections=100，同机 api 侧 10，
    // 合起来仍留足头寸。真正的护栏是 locks.rs 里的槽位（池的一半），
    // 这个数只是让「一半」也够用。
    let pool_size: u32 = std::env::var("DB_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(JOB_POOL_DEFAULT);
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(pool_size)
        .connect(&db_url)
        .await?;
    // 运行日志落库（0218 G6）：挂在这一刻之后的事件进 runtime_logs
    runtime_log::attach(db.clone());
    let redis = redis::Client::open(redis_url.as_str())?
        .get_connection_manager()
        .await?;

    tracing::info!("flux-worker started");
    // 0416 每日结算凭证：独立循环（run.rs 已是存量超限文件，只许瘦）
    tokio::spawn(jobs::spark_merkle_loop(db.clone()));
    jobs::run_all(db, redis).await
}

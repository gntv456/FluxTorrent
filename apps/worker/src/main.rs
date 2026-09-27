//! FluxTorrent Worker：异步任务组（§5.2）。
//! 职责：announce 计费消费、促销到期回收、保种区移出、做种收益结算、签到连签、
//!       银行每日结算、任务达标结算、考核自动派发。

mod bank_jobs;
mod jobs;
mod runtime_log;
mod shutdown;
mod task_jobs;

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

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://flux:flux@127.0.0.1:5432/fluxtorrent".into()
    });
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    // 0225 G30-B14：并发调度后同 tick 最多 10 定时 + 3 手动任务并发抢连接，
    // 固定 5 会 PoolTimedOut（且被 run_guarded 的笼统文案误报成锁冲突）。
    // 缺省提到 16，可经 DB_POOL_SIZE 覆盖。
    let pool_size: u32 = std::env::var("DB_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
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
    jobs::run_all(db, redis).await
}

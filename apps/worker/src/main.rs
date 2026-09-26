//! FluxTorrent Worker：异步任务组（§5.2）。
//! 职责：announce 计费消费、促销到期回收、保种区移出、做种收益结算、签到连签、
//!       银行每日结算、任务达标结算、考核自动派发。

mod bank_jobs;
mod jobs;
mod runtime_log;
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

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://flux:flux@127.0.0.1:5432/fluxtorrent".into()
    });
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
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

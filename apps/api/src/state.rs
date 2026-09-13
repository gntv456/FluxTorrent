//! 应用共享状态：DB 池 + Redis + 配置。

use crate::config::AppConfig;
use crate::repo::Repo;

pub struct AppState {
    pub cfg: AppConfig,
    pub repo: Repo,
    pub redis: redis::aio::ConnectionManager,
    /// M28 插件管理器（编译期装配，运行期启停由插件 enabled() 决定）
    pub plugins: crate::plugins::PluginManager,
    /// 进程启动时间（stats 页 uptime 口径）
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub async fn new(cfg: AppConfig) -> anyhow::Result<Self> {
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(cfg.db_pool_size as u32)
            .connect(&cfg.database_url)
            .await?;
        // ConnectionManager 内建无限重连：认证失败/不可达时不返回错误，
        // 进程会永远停在启动阶段（容器表现为 running 但无日志、不监听）。用
        // 10s 超时把「连不上」变成启动失败，避免带病挂活。
        let redis = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            redis::Client::open(cfg.redis_url.as_str())?.get_connection_manager(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("连接 Redis 超时（10s）：{}", cfg.redis_url))??;
        Ok(Self {
            cfg,
            repo: Repo::new(db),
            redis,
            plugins: crate::plugins::PluginManager::builtin(),
            started_at: chrono::Utc::now(),
        })
    }
}

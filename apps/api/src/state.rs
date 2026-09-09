//! 应用共享状态：DB 池 + Redis + 配置。

use crate::config::AppConfig;
use crate::repo::Repo;

pub struct AppState {
    pub cfg: AppConfig,
    pub repo: Repo,
    pub redis: redis::aio::ConnectionManager,
    /// M28 插件管理器（编译期装配，运行期启停由插件 enabled() 决定）
    pub plugins: crate::plugins::PluginManager,
}

impl AppState {
    pub async fn new(cfg: AppConfig) -> anyhow::Result<Self> {
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(cfg.db_pool_size as u32)
            .connect(&cfg.database_url)
            .await?;
        let redis = redis::Client::open(cfg.redis_url.as_str())?
            .get_connection_manager()
            .await?;
        Ok(Self {
            cfg,
            repo: Repo::new(db),
            redis,
            plugins: crate::plugins::PluginManager::builtin(),
        })
    }
}

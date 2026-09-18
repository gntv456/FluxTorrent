//! 应用共享状态：DB 池 + Redis + 配置。

use crate::config::AppConfig;
use crate::repo::Repo;

pub struct AppState {
    /// 应用配置（JWT 密钥等敏感项已在启动期消化进 jwt；字段保留供后续配置类功能读取）
    #[allow(dead_code)]
    pub cfg: AppConfig,
    pub repo: Repo,
    pub redis: redis::aio::ConnectionManager,
    /// JWT 签发器（0071：hs256 / rs256 由配置决定，启动时构造一次）
    pub jwt: crate::auth::JwtSigner,
    /// M28 插件管理器（编译期装配，运行期启停由插件 enabled() 决定）
    pub plugins: crate::plugins::PluginManager,
    /// 模块开关缓存（U1 §5.1：TTL 30s + 后台改键主动失效；worker 与 API 共用）
    pub module_flags: crate::modules::ModuleFlags,
    /// 进程启动时间（stats 页 uptime 口径）
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub async fn new(cfg: AppConfig) -> anyhow::Result<Self> {
        let jwt = crate::auth::JwtSigner::from_config(&cfg.jwt_alg, &cfg.jwt_secret)?;
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
            jwt,
            plugins: crate::plugins::PluginManager::builtin(),
            module_flags: crate::modules::ModuleFlags::new(),
            started_at: chrono::Utc::now(),
        })
    }
}

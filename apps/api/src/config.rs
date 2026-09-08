use serde::Deserialize;

/// API 服务配置：全部经环境变量注入（§8.1 环境变量边界），禁止硬编码密钥。
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    pub database_url: String,
    #[serde(default = "default_redis")]
    pub redis_url: String,
    /// JWT RS256 私钥（PEM）。生产由密钥管理注入；开发态可退化为 HS256 secret。
    pub jwt_secret: String,
    #[serde(default = "default_pool")]
    pub db_pool_size: usize,
    #[serde(default = "default_true")]
    #[allow(dead_code)]
    pub seed_demo_data: bool,
}

fn default_bind() -> String {
    "0.0.0.0:8080".into()
}
fn default_redis() -> String {
    "redis://127.0.0.1:6379".into()
}
fn default_pool() -> usize {
    10
}
fn default_true() -> bool {
    true
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        envy::from_env::<AppConfig>().map_err(|e| anyhow::anyhow!("config: {e}"))
    }
}

use serde::Deserialize;

/// API 服务配置：全部经环境变量注入（§8.1 环境变量边界），禁止硬编码密钥。
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    pub database_url: String,
    #[serde(default = "default_redis")]
    pub redis_url: String,
    /// JWT 密钥：hs256 模式为共享密钥（≥32B）；rs256 模式仅作回退占位（实际用 RSA PEM）。
    #[serde(default)]
    pub jwt_secret: String,
    /// JWT 算法（0071）：`hs256`（默认，兼容存量部署）或 `rs256`（生产推荐，私钥只在 api 进程）。
    #[serde(default = "default_jwt_alg")]
    pub jwt_alg: String,
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
fn default_jwt_alg() -> String {
    "hs256".into()
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let cfg = envy::from_env::<AppConfig>()
            .map_err(|e| anyhow::anyhow!("config: {e}"))?;
        let alg = cfg.jwt_alg.to_ascii_lowercase();
        if alg != "hs256" && alg != "rs256" {
            return Err(anyhow::anyhow!(
                "JWT_ALG 仅支持 hs256 / rs256（当前 {alg}）"
            ));
        }
        // §5.7：HS256 密钥 ≥32 字节；含 "change_me" 的占位密钥禁止在非开发态使用。
        // rs256 模式密钥由 RSA PEM 提供，不再要求 JWT_SECRET。
        if alg == "hs256" {
            if cfg.jwt_secret.len() < 32 {
                return Err(anyhow::anyhow!(
                    "JWT_SECRET 长度不足 32 字节（当前 {}）",
                    cfg.jwt_secret.len()
                ));
            }
            let dev = std::env::var("FLUX_DEV").unwrap_or_default() == "1";
            if !dev && cfg.jwt_secret.contains("change_me") {
                return Err(anyhow::anyhow!(
                    "生产环境禁止使用含 change_me 的占位 JWT_SECRET（设 FLUX_DEV=1 跳过开发态检查）"
                ));
            }
        }
        Ok(cfg)
    }
}

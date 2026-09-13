//! JWT 签发与校验（§5.7）。
//! 0071 安全升级：支持 RS256（`JWT_ALG=rs256`）——私钥只在 api 进程，任何拿到日志/CI 的
//! 组件都无法再凭 `JWT_SECRET` 伪造任意用户 token；缺省仍为 HS256（≥32B）保持开发态兼容。
//! 切换算法会使全部已签发 token 失效（用户重新登录），需在停机窗口操作。

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64, // user_id
    pub class_id: i32,
    pub exp: i64,
    pub iat: i64,
}

/// 签发器：应用启动时按配置构造一次，运行期零分支。
pub enum JwtSigner {
    Hs { secret: String },
    Rs { enc: EncodingKey, dec: DecodingKey },
}

impl JwtSigner {
    /// alg：`hs256`（默认）或 `rs256`。
    /// RS256 密钥来源：`JWT_RS_PRIVATE_PEM` / `JWT_RS_PUBLIC_PEM`（PEM 文本），
    /// 未配置时自动生成 2048 位密钥对并落盘 `JWT_RS_KEY_DIR`（默认 `data/`），
    /// 重启复用；生产建议由密钥管理注入而非自动生成。
    pub fn from_config(alg: &str, secret: &str) -> anyhow::Result<Self> {
        match alg.to_ascii_lowercase().as_str() {
            "rs256" => {
                let (priv_pem, pub_pem) = load_or_generate_rs_pem()?;
                Ok(JwtSigner::Rs {
                    enc: EncodingKey::from_rsa_pem(priv_pem.as_bytes())
                        .map_err(|e| anyhow::anyhow!("JWT_RS 私钥解析失败: {e}"))?,
                    dec: DecodingKey::from_rsa_pem(pub_pem.as_bytes())
                        .map_err(|e| anyhow::anyhow!("JWT_RS 公钥解析失败: {e}"))?,
                })
            }
            _ => {
                if secret.len() < 32 {
                    return Err(anyhow::anyhow!("HS256 模式要求 JWT_SECRET ≥32 字节"));
                }
                Ok(JwtSigner::Hs {
                    secret: secret.to_string(),
                })
            }
        }
    }

    pub fn issue(&self, user_id: i64, class_id: i32, ttl_hours: i64) -> anyhow::Result<String> {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            class_id,
            iat: now,
            exp: now + ttl_hours * 3600,
        };
        match self {
            JwtSigner::Hs { secret } => encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(secret.as_bytes()),
            ),
            JwtSigner::Rs { enc, .. } => encode(&Header::new(Algorithm::RS256), &claims, enc),
        }
        .map_err(|e| anyhow::anyhow!("jwt issue: {e}"))
    }

    pub fn verify(&self, token: &str) -> Option<Claims> {
        match self {
            JwtSigner::Hs { secret } => decode::<Claims>(
                token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &Validation::default(),
            ),
            JwtSigner::Rs { dec, .. } => {
                decode::<Claims>(token, dec, &Validation::new(Algorithm::RS256))
            }
        }
        .ok()
        .map(|d| d.claims)
    }
}

/// RS256 密钥加载/生成（P1：生产环境由密钥管理系统注入 PEM，开发态自动落盘）。
fn load_or_generate_rs_pem() -> anyhow::Result<(String, String)> {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    let env_priv = std::env::var("JWT_RS_PRIVATE_PEM").unwrap_or_default();
    let env_pub = std::env::var("JWT_RS_PUBLIC_PEM").unwrap_or_default();
    if !env_priv.is_empty() && !env_pub.is_empty() {
        return Ok((env_priv, env_pub));
    }
    let binding = std::env::var("JWT_RS_KEY_DIR").unwrap_or_else(|_| "data".into());
    let dir = std::path::Path::new(&binding);
    let priv_path = dir.join("jwt_rs_private.pem");
    let pub_path = dir.join("jwt_rs_public.pem");
    if let (Ok(priv_pem), Ok(pub_pem)) = (
        std::fs::read_to_string(&priv_path),
        std::fs::read_to_string(&pub_path),
    ) {
        return Ok((priv_pem, pub_pem));
    }
    tracing::warn!(
        "未配置 JWT_RS_PRIVATE_PEM/JWT_RS_PUBLIC_PEM：自动生成 2048 位密钥对并写入 {}（生产环境请改用密钥管理注入）",
        dir.display()
    );
    let mut rng = rand::thread_rng();
    let key = rsa::RsaPrivateKey::new(&mut rng, 2048)
        .map_err(|e| anyhow::anyhow!("RSA 密钥生成失败: {e}"))?;
    let priv_pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| anyhow::anyhow!("私钥序列化失败: {e}"))?
        .to_string();
    let pub_pem = key
        .to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| anyhow::anyhow!("公钥序列化失败: {e}"))?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(&priv_path, &priv_pem)?;
    std::fs::write(&pub_path, &pub_pem)?;
    Ok((priv_pem, pub_pem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_roundtrip_hs() {
        let signer = JwtSigner::from_config("hs256", "0123456789abcdef0123456789abcdef").unwrap();
        let t = signer.issue(42, 3, 1).unwrap();
        let c = signer.verify(&t).unwrap();
        assert_eq!(c.sub, 42);
        assert_eq!(c.class_id, 3);
        let bad = JwtSigner::from_config("hs256", "wrong-secret-xxxxxxxxxxxxxxxxxxxxxx").unwrap();
        assert!(bad.verify(&t).is_none());
    }

    #[test]
    fn jwt_roundtrip_rs() {
        let signer = JwtSigner::from_config("rs256", "").unwrap();
        let t = signer.issue(7, 5, 1).unwrap();
        let c = signer.verify(&t).unwrap();
        assert_eq!(c.sub, 7);
        assert_eq!(c.class_id, 5);
        // HS256 签发的 token 不能被 RS256 校验（算法混淆防护由 Validation::new(算法) 保证）
        let hs = JwtSigner::from_config("hs256", "0123456789abcdef0123456789abcdef").unwrap();
        let hs_token = hs.issue(7, 5, 1).unwrap();
        assert!(signer.verify(&hs_token).is_none());
    }
}

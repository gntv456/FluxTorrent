//! JWT 签发与校验（§5.7：RS256/HS256+256bit，此处用 HS256≥32B 开发态，生产换 RS256）。

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64, // user_id
    pub class_id: i32,
    pub exp: i64,
    pub iat: i64,
}

pub fn issue(user_id: i64, class_id: i32, secret: &str, ttl_hours: i64) -> anyhow::Result<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        class_id,
        iat: now,
        exp: now + ttl_hours * 3600,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("jwt issue: {e}"))
}

pub fn verify(token: &str, secret: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_roundtrip() {
        let secret = "0123456789abcdef0123456789abcdef";
        let t = issue(42, 3, secret, 1).unwrap();
        let c = verify(&t, secret).unwrap();
        assert_eq!(c.sub, 42);
        assert_eq!(c.class_id, 3);
        assert!(verify(&t, "wrong-secret-xxxxxxxxxxxxxxxxx").is_none());
    }
}

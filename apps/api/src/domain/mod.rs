//! 领域层：用户与认证（M01）。
//! 业务规则唯一事实源（§8.1）：邀请码校验、密码策略、passkey 生成。

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use rand::Rng;

use crate::errors::{DomainError, DomainResult};

#[allow(dead_code)]
pub const INVITE_TTL_HOURS: i64 = 72;

pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[allow(dead_code)]
pub struct UserAccount {
    pub id: i64,
    pub username: String,
    pub pass_hash: String,
    pub passkey: String,
    pub class_id: i32,
    pub must_reset_password: bool,
}

/// Argon2id 哈希（§5.7：禁止 MD5/SHA1 裸存）
pub fn hash_password(password: &str) -> DomainResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| DomainError::Internal(anyhow::anyhow!("hash: {e}")))
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    argon2::PasswordVerifier::verify_password(
        &argon2::Argon2::default(),
        password.as_bytes(),
        &argon2::PasswordHash::new(hash)
            .unwrap_or_else(|_| argon2::PasswordHash::new("$argon2id$").unwrap()),
    )
    .is_ok()
}

pub fn new_passkey() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

#[allow(dead_code)]
pub fn new_invite_code() -> String {
    new_passkey()
}

/// 注册入参校验（权威校验在后端，前端仅体验级 §8.1）
pub fn validate_register(u: &NewUser) -> DomainResult<()> {
    let username_len = u.username.chars().count();
    if !(3..=24).contains(&username_len) {
        return Err(DomainError::Validation("用户名长度需 3-24 字符".into()));
    }
    if !u
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || (c as u32) > 0x4e00)
    {
        return Err(DomainError::Validation(
            "用户名仅允许字母数字下划线或中文".into(),
        ));
    }
    if !u.email.contains('@') || u.email.len() < 5 {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    if u.password.len() < 8 {
        return Err(DomainError::Validation("密码至少 8 位".into()));
    }
    Ok(())
}

/// 邀请码到期判定（旧站口径 72h，LIMITS.INVITE_TTL_HOURS）
#[allow(dead_code)]
pub fn invite_expired(expires_at: chrono::DateTime<Utc>) -> bool {
    Utc::now() > expires_at
}

#[allow(dead_code)]
pub fn invite_expiry() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::hours(INVITE_TTL_HOURS)
}

/// 促销裁决（M06 §5.5）：单种子促销覆盖全站，取折扣更优者。
/// 纯函数，可穷举单测。
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromotionKind {
    None,
    Free,
    X2,
    X2Free,
    Half,
    X2Half,
    P30,
}

#[allow(dead_code)]
impl PromotionKind {
    /// 计费倍率 (up_mult, down_mult)
    pub fn multipliers(self) -> (f64, f64) {
        match self {
            PromotionKind::None => (1.0, 1.0),
            PromotionKind::Free => (1.0, 0.0),
            PromotionKind::X2 => (2.0, 1.0),
            PromotionKind::X2Free => (2.0, 0.0),
            PromotionKind::Half => (1.0, 0.5),
            PromotionKind::X2Half => (2.0, 0.5),
            PromotionKind::P30 => (1.0, 0.3),
        }
    }

    /// 促销强度序（用于「折扣最大」裁决）
    fn strength(self) -> u8 {
        match self {
            PromotionKind::None => 0,
            PromotionKind::P30 => 1,
            PromotionKind::Half => 2,
            PromotionKind::Free => 3,
            PromotionKind::X2 => 4,
            PromotionKind::X2Half => 5,
            PromotionKind::X2Free => 6,
        }
    }
}

#[allow(dead_code)]
pub fn resolve_promotion(
    torrent_level: PromotionKind,
    global_level: PromotionKind,
) -> PromotionKind {
    if torrent_level.strength() >= global_level.strength() {
        torrent_level
    } else {
        global_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_resolution_prefers_stronger() {
        assert_eq!(
            resolve_promotion(PromotionKind::Free, PromotionKind::X2),
            PromotionKind::X2
        );
        assert_eq!(
            resolve_promotion(PromotionKind::X2Free, PromotionKind::Free),
            PromotionKind::X2Free
        );
        assert_eq!(
            resolve_promotion(PromotionKind::None, PromotionKind::Free),
            PromotionKind::Free
        );
    }

    #[test]
    fn multipliers_table() {
        assert_eq!(PromotionKind::Free.multipliers(), (1.0, 0.0));
        assert_eq!(PromotionKind::X2Half.multipliers(), (2.0, 0.5));
    }

    #[test]
    fn passkey_is_32_chars() {
        assert_eq!(new_passkey().len(), 32);
    }

    #[test]
    fn password_roundtrip() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password(&h, "correct horse"));
        assert!(!verify_password(&h, "wrong"));
    }

    #[test]
    fn register_validation() {
        let ok_user = NewUser {
            username: "alice".into(),
            email: "a@b.com".into(),
            password: "12345678".into(),
        };
        assert!(validate_register(&ok_user).is_ok());
        let bad = NewUser {
            username: "a".into(),
            email: "a@b.com".into(),
            password: "12345678".into(),
        };
        assert!(matches!(
            validate_register(&bad),
            Err(DomainError::Validation(_))
        ));
    }
}

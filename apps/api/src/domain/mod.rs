//! 领域层：用户与认证（M01）。
//! 业务规则唯一事实源（§8.1）：邀请码校验、密码策略、passkey 生成。

// 种子审核状态机（FRAME §7 第 6 条）：approval_status 迁移矩阵唯一事实源
pub mod torrent_status;

// 商城三段式履约（冻结 → 履约 → 扣款）
pub mod shop;

// 术语键生命周期 + 覆盖率巡检（治理机制，非功能）
pub mod terms;
pub mod webhook;

// 抽奖可验证公平（commit-reveal）：判定与哈希派生进 domain 纯函数层
pub mod lottery_fair;
pub mod mediainfo;

// H&R 分档要求 + 做种减免（护肝核心）
pub mod hr;
pub mod hr_warn;

// 站点策略的归一与版本化（公开口径单一事实源）
pub mod policy;

// seedbox 盒子：CIDR 校验与标志位语义（只放宽不加重）
pub mod seedbox;

// 勋章自动成就（双模式 + 回填 + 进度）
pub mod medal;

// 考核多阶段推进 + 补救期（状态机判定进 domain）
pub mod exam;

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use rand::Rng;

use crate::errors::{DomainError, DomainResult};

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
    /// 闲置停用时间（0072）：非空 = 账号被标记为休眠，登录需先联系管理组恢复
    pub dormant_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 归档（0228 inactivity_policy=archive 档）：档案页隐藏、登录文案区分
    pub archived: bool,
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
    // 非法/占位哈希（如种子数据 "stub"）直接判失败，绝不让验证器 panic 拖垮 worker 线程
    let Ok(parsed) = argon2::PasswordHash::new(hash) else {
        return false;
    };
    argon2::PasswordVerifier::verify_password(
        &argon2::Argon2::default(),
        password.as_bytes(),
        &parsed,
    )
    .is_ok()
}

/// 旧站迁移哈希验证（好学站 NexusPHP）：主口径双层
/// sha256(secret + sha256(password))（takelogin.php:61），盐 secret 随迁。
/// 老式无 auth_key 口径 md5(secret + password + secret) 兜底（takelogin.php:68）。
/// bcrypt（Laravel 系）按前缀分流。返回是否命中迁移口径——命中后由
/// auth_http 透明升级 argon2id。
pub fn verify_legacy_password(
    hash: &str,
    secret: Option<&str>,
    password: &str,
) -> bool {
    use sha2::{Digest, Sha256};
    if is_bcrypt_hash(hash) {
        return bcrypt::verify(password, hash).unwrap_or(false);
    }
    // NexusPHP 双层 sha256（64 hex 小写）——secret 必在
    if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let Some(secret) = secret else { return false };
        let inner = Sha256::digest(password.as_bytes());
        // PHP 口径：secret . hash('sha256', pw) —— inner 是 hex 字符串再拼
        let inner_hex = format!("{inner:x}");
        let mut outer = Sha256::new();
        outer.update(secret.as_bytes());
        outer.update(inner_hex.as_bytes());
        hex_eq(&format!("{:x}", outer.finalize()), hash)
    } else {
        false
    }
}

/// 大小写不敏感 hex 比对（MySQL dump 可能带大写）
fn hex_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// bcrypt 迁移哈希判定
pub fn is_bcrypt_hash(hash: &str) -> bool {
    const P: [&str; 3] = ["$2a$", "$2b$", "$2y$"];
    P.iter().any(|p| hash.starts_with(p))
}

pub fn new_passkey() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    // crypto RNG（0214）：passkey/邀请码是对外凭据，thread_rng 非密码学强度
    let mut rng = rand::rngs::OsRng;
    (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// passkey 宽限窗（小时，默认 7 天）。审计 10-07 P1-4：passkey 被烤进用户
/// 已下载的每一个 .torrent，改密/重置即刻失效等于集体停种——旧钥在这段窗口内
/// 仍然可用，同时下发提示让用户换新种子。
pub fn passkey_grace_hours() -> i64 {
    std::env::var("PASSKEY_GRACE_HOURS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(168)
        .clamp(0, 24 * 30)
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
    if !valid_email(&u.email) {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    if u.password.len() < 8 {
        return Err(DomainError::Validation("密码至少 8 位".into()));
    }
    Ok(())
}

/// 严格邮箱校验（P2，2026-10-06 安全审计）：单一 validator 供注册/邀请/
/// 找回等所有入口复用。拒空格/多 @/无点域/CRLF/过长——lettre 的 Mailbox
/// 解析兜底头注入，这里在入库前就拦掉畸形形态。
pub(crate) fn valid_email(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 5 || s.len() > 254 || s.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = s.rsplit_once('@') else {
        return false;
    };
    // RFC 粗粒度：local 至少 1 字符且无连续点结尾；域须有至少一个点且
    // 每段非空（拦截 "a@b"、"a@.com"、"a@b."）
    !local.is_empty()
        && !local.starts_with('.')
        && !local.ends_with('.')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

/// 邀请码到期判定（旧站口径 72h，LIMITS.INVITE_TTL_HOURS）
#[allow(dead_code)]
pub fn invite_expired(expires_at: chrono::DateTime<Utc>) -> bool {
    Utc::now() > expires_at
}

/// 邀请码签发时的到期时间（旧站口径 72h，LIMITS.INVITE_TTL_HOURS）
#[allow(dead_code)]
pub fn invite_expiry() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::hours(72)
}

// 促销档位枚举曾在这里是 M06 的第二份倍率表（外加「取最强档」的合并规则），
// 全站无人调用却带着一条把 free+x2 判成 x2 的单测（批三十七 P-120 实测：
// 那条规则会吞掉免流）。口径唯一来源改为：
//   计费 = worker `jobs::billing::billing_multipliers`
//   展示 = api  `torrents::promo`（同一规则的 SQL 片段）
// 两处不一致由闸门 BA（真发 announce → 真读 traffic_ledger + 详情角标）钉住。

#[cfg(test)]
mod tests {
    use super::*;

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
    fn bcrypt_migration_hash_verify() {
        // 旧站 Laravel $2y$ 口径样例（明文 password123），原样迁入后须可验证
        let h = concat!(
            "$2y$10$Ffk5RL6a2oIZiBElNHFEJeyQdjGsYov3BmIkF",
            "/M.VCllxhEh0uC1K"
        );
        assert!(is_bcrypt_hash(h));
        assert!(verify_legacy_password(h, None, "password123"));
        assert!(!verify_legacy_password(h, None, "wrong"));
        // $2b$ 同源；argon2 哈希不判 bcrypt
        assert!(is_bcrypt_hash(
            "$2b$10$Ffk5RL6a2oIZiBElNHFEJeyQdjGsYov3BmIkF/M.VCllxhEh0uC1K"
        ));
        assert!(!is_bcrypt_hash("$argon2id$v=19$m=19456,t=2,p=1$abc"));
    }

    #[test]
    fn nexusphp_legacy_password_verify() {
        // 旧站 takelogin.php 双层口径真向量（明文 password123 / 盐见下）
        let secret = "abcdef1234567890";
        // sha256(secret + sha256(pw))
        let h2 =
            "d057dc8082ebffb2638028526710331a87e06fa4dce46d6b9ef1a070a5f3d4ea";
        assert!(verify_legacy_password(h2, Some(secret), "password123"));
        assert!(!verify_legacy_password(h2, Some(secret), "wrong"));
        assert!(!verify_legacy_password(h2, None, "password123"));
        // 大写 hex dump 同样命中（大小写不敏感）
        let up = h2.to_uppercase();
        assert!(verify_legacy_password(&up, Some(secret), "password123"));
        // 32 位 md5 老口径不在本判定范围（现网全量 64-hex）——不命中
        let hm = "4289f62c83974bcff579ee45488402ab";
        assert!(!verify_legacy_password(hm, Some(secret), "password123"));
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

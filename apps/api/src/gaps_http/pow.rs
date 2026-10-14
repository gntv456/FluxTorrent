//! 登录自托管 PoW 闸门（sha256-pow-v1）。
//!
//! 动机：账户锁定（5 次锁 15 分钟）拦「持续爆破」，登录限流拦高频，但
//! 「第 3~5 次尝试」这段窗口内攻击者仍可零成本试密码。PoW 让每次尝试
//! 先付一段 CPU（difficulty 18bit ≈ 几万次 SHA-256，浏览器秒级），
//! 对单账户爆破的成本从 0 抬到与合法用户同等算力。
//!
//! 协议：
//! ① `GET /auth/pow-challenge?username=` —— 仅当该用户 acctlock 失败
//!    计数 ≥2（第 3 次尝试起）才签发，防无谓挑战与挑战接口被刷；
//! ② 32 字节随机 hex challenge，Redis `sha3/pow:{token}` 存 5min
//!    （set_ex 仿 captcha.rs），值为 `{challenge}:{difficulty}`；
//! ③ 登录体带 pow_token/pow_solution，GETDEL 一次性取回后校验：
//!    `SHA-256("{challenge}:{solution}")` hex 展开前 difficulty 个 bit
//!    为 0（difficulty/4 个 '0' 前缀字符 + 余数位掩码）；
//! ④ 校验失败与密码错同点 counter_bump；成功登录/锁定清零时 DEL。
//!
//! difficulty 站点可配（site_settings.pow_difficulty_bits，迁移 0353
//! 登记），缺省 18，钳 12..=24。

use actix_web::{get, web, HttpRequest, Responder};
use redis::AsyncCommands;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 键前缀（token 一次性，GETDEL 消费）
const POW_KEY: &str = "sha3/pow:";
/// 挑战 TTL（秒）：签发后 5 分钟内有效，过期重取
const POW_TTL: u64 = 300;
/// 触发线：acctlock 失败计数达到该值（即第 3 次尝试）起要求 PoW
pub(crate) const POW_FAILS_FROM: i64 = 2;
/// 缺省难度（bit）：18 ≈ 平均 26 万次哈希，浏览器亚秒级
const POW_DIFFICULTY_DEFAULT: i32 = 18;

#[derive(Deserialize)]
struct PowQuery {
    username: String,
}

/// 难度读站点设定：pow_difficulty_bits，缺省 18，钳 12..=24
async fn difficulty(db: &sqlx::PgPool) -> i32 {
    sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = \
         'pow_difficulty_bits'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v: String| v.parse::<i32>().ok())
    .unwrap_or(POW_DIFFICULTY_DEFAULT)
    .clamp(12, 24)
}

/// GET /auth/pow-challenge：仅失败计数 ≥2 的用户名签发（防无谓挑战）
#[get("/auth/pow-challenge")]
pub async fn pow_challenge(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<PowQuery>,
) -> DomainResult<impl Responder> {
    // 防刷限流（同 captcha 口径）：挑战签发本身也要付 IP 维度成本
    let ip = crate::http::client_ip(&req);
    crate::http::rate_window(&state, &format!("pow:{ip}"), 30, 60).await?;
    let name = q.username.trim().to_lowercase();
    let fails =
        crate::http::counter_peek(&state, &format!("acctlock:{name}")).await?;
    if fails < POW_FAILS_FROM {
        // 未达触发线不发题：接口对「任意用户名」无探测价值
        return Err(DomainError::Validation("当前无需工作量证明".into()));
    }
    // 32 字节 crypto 随机 → 64 hex（对外凭据口径同 new_passkey 的 OsRng）
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let challenge: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let token = uuid::Uuid::new_v4().to_string();
    let bits = difficulty(&state.repo.db).await;
    let mut c = state.redis.clone();
    let val = format!("{challenge}:{bits}");
    let _: () = AsyncCommands::set_ex(
        &mut c,
        format!("{POW_KEY}{token}"),
        val,
        POW_TTL,
    )
    .await
    .map_err(|e| {
        DomainError::Internal(anyhow::anyhow!("PoW 存储不可用: {e}"))
    })?;
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(300);
    Ok(ok(serde_json::json!({
        "pow_token": token,
        "pow_challenge": challenge,
        "pow_difficulty": bits,
        "pow_expires_at": expires_at.to_rfc3339(),
    })))
}

/// 校验入口（login.rs 在失败窗口内调用）：
/// GETDEL 取回（一次性）→ hash 前缀判 leading zero bits。
/// 返回 Ok(()) = 通过；Err = token 不存在/过期或解不满足难度。
pub(crate) async fn pow_verify(
    state: &std::sync::Arc<AppState>,
    token: &str,
    solution: &str,
) -> DomainResult<()> {
    // token/solution 形状闸：防把任意串塞进 Redis 键名或哈希输入
    if token.is_empty()
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || solution.len() > 128
        || !solution
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(DomainError::Validation("PoW 参数形状无效".into()));
    }
    let mut c = state.redis.clone();
    let val: Option<String> = redis::cmd("GETDEL")
        .arg(format!("{POW_KEY}{token}"))
        .query_async(&mut c)
        .await
        .map_err(|e| {
            DomainError::Internal(anyhow::anyhow!("PoW 存储不可用: {e}"))
        })?;
    let Some(v) = val else {
        return Err(DomainError::Validation(
            "PoW 令牌无效或已使用，请重新获取挑战".into(),
        ));
    };
    let Some((challenge, bits)) = v.split_once(':') else {
        return Err(DomainError::Validation("PoW 挑战损坏".into()));
    };
    let Ok(bits) = bits.parse::<u32>() else {
        return Err(DomainError::Validation("PoW 挑战损坏".into()));
    };
    if !meets_difficulty(&pow_hash(challenge, solution), bits) {
        return Err(DomainError::Validation("PoW 解不满足难度要求".into()));
    }
    Ok(())
}

/// 删除该用户的全部挂起挑战（成功登录/锁定清零时；失败只记 warn 同 counter_clear）
pub(crate) async fn pow_clear(state: &std::sync::Arc<AppState>, token: &str) {
    if token.is_empty() {
        return;
    }
    let mut c = state.redis.clone();
    let touched: redis::RedisResult<()> =
        c.del(format!("{POW_KEY}{token}")).await;
    if let Err(e) = touched {
        tracing::warn!(%e, "PoW 挑战键清理失败（会按 TTL 自然过期）");
    }
}

/// 密码/TOTP 错误路径的 PoW 记账挂点（login.rs 两处 bump 之后调用）。
/// PoW 校验失败已在入口闸门处与密码错同点 counter_bump 并拒绝（见
/// login.rs），到达这里的失败都已通过 PoW——无事可做，保留显式挂点
/// 是让「校验失败也计数」的口径在两个失败分支可见、可审。
pub(crate) fn pow_noop() {}

/// SHA-256("<challenge>:<solution>") 的 hex 展开
pub(super) fn pow_hash(challenge: &str, solution: &str) -> String {
    let d = Sha256::digest(format!("{challenge}:{solution}"));
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// hex 摘要前 `bits` 个 bit 是否全 0：
/// 前 bits/4 个字符须为 '0'，余 bits%4 个 bit 用末字符高位掩码判
pub(super) fn meets_difficulty(hex: &str, bits: u32) -> bool {
    if bits == 0 || bits as usize > hex.len() * 4 {
        return false;
    }
    let full = (bits / 4) as usize;
    if hex[..full].chars().any(|c| c != '0') {
        return false;
    }
    let rem = bits % 4;
    if rem == 0 {
        return true;
    }
    // 前 rem 个高位 bit 为 0 ⇒ 首个非零字符 <= 0xF >> rem
    let mask = 0xFu8 >> rem;
    let next = hex.as_bytes()[full];
    next.is_ascii_digit() && (next - b'0') <= mask
        || next.is_ascii_lowercase() && (next - b'a' + 10) <= mask
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 已知 challenge+difficulty 构造合法 solution 校验通过
    #[test]
    fn accepts_valid_solution() {
        let ch = "ab".repeat(32);
        // 找一个前 4 hex 位为 0 的解（difficulty=16）
        let sol = (0..1_000_000u64)
            .find(|&n| meets_difficulty(&pow_hash(&ch, &n.to_string()), 16))
            .unwrap()
            .to_string();
        assert!(meets_difficulty(&pow_hash(&ch, &sol), 16));
    }

    /// 前导零不足拒绝
    #[test]
    fn rejects_insufficient_leading_zeros() {
        let ch = "cd".repeat(32);
        // 前 20 bit 全 0 概率 2^-20，普通 hash 必然不满足
        for n in 0..10u64 {
            let h = pow_hash(&ch, &n.to_string());
            if h[..5].chars().any(|c| c != '0') {
                assert!(!meets_difficulty(&h, 20));
            }
        }
    }

    /// 难度掩码位对齐（非 4 整数倍 bit 数）
    #[test]
    fn mask_semantics_non_multiple_of_four() {
        // 12 bit：前 3 个 '0'（第 4 字符任意）
        assert!(meets_difficulty("0008000", 12));
        assert!(!meets_difficulty("0018000", 12));
        // 13 bit：第 4 字符值须 <= 0xF>>1 = 7
        assert!(!meets_difficulty("0008000", 13));
        assert!(meets_difficulty("0007000", 13));
        // 16 / 17 bit 边界
        assert!(!meets_difficulty("0008fff", 16));
        assert!(meets_difficulty("00007ff", 17));
        assert!(!meets_difficulty("00008ff", 17));
        // 0 bit 拒绝（必须显式难度）
        assert!(!meets_difficulty("0000", 0));
    }
}

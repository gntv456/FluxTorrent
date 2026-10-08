//! 兼容层共享工具：SHA3-256 哈希十六进制输出（passkey / 凭证哈希与限流桶）。

use actix_web::web;
use sha3::{Digest, Sha3_256};

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// passkey 端点的同 IP 每分钟探测上限（防换 passkey 绕过分桶的资源消耗面）：
/// 覆盖正常用户多 passkey 端点的合理使用，见 limit_passkey 二轮补注。
pub(crate) const PASSKEY_IP_PER_MIN: i64 = 120;

pub(super) fn sha3_hex(bytes: &[u8]) -> String {
    let mut h = Sha3_256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// passkey 形状校验（与 `domain::new_passkey` 同口径：32 位小写字母数字）。
///
/// 别名路径要把 passkey 拼进 302 的 `Location` 头 —— 严格校验顺带堵死
/// 「畸形输入被搬进响应头」这类边角（合法字符集下不可能出现 CR/LF 或引号）。
pub(crate) fn valid_passkey(p: &str) -> bool {
    p.len() == 32
        && p.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// passkey 维度限流：桶 = sha3(passkey) 前 16 位，60s 固定窗口。
///
/// 0267 补：此前只有 `download.php` 做了 passkey 限流，而 `/rss/{passkey}`、
/// `getrss.php`、`userdetails.php` 全都**无限流** —— 一枚泄露的 passkey 可以
/// 无限次拉订阅源与账号信息（打库 + 放大信息面）。凡凭 passkey 鉴权的对外端点
/// 都应节流，与 download.php 同口径。
/// Redis 故障时 fail-close（与 http::throttle 一致，不给抖动期留缺口）。
pub(crate) async fn limit_passkey(
    state: &web::Data<std::sync::Arc<AppState>>,
    req: &actix_web::HttpRequest,
    scope: &str,
    passkey: &str,
    max: i64,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let bucket = sha3_hex(passkey.as_bytes());
    let key = format!("rl:{scope}:{}", &bucket[..16]);
    let mut c = state.redis.clone();
    let n: i64 = c.incr(&key, 1).await.map_err(|e| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用: {e}"))
    })?;
    if n == 1 {
        let _: () = c.expire(&key, 60).await.unwrap_or(());
    }
    if n > max {
        return Err(DomainError::RateLimited);
    }
    // 二轮遗留（2026-10-07）：IP 第二维——旧版只按被试 passkey 分桶，攻击者
    // 每次换随机 passkey 即新桶 = 等效无限流（匿名无界打 users 索引查询）。
    // passkey 空间 36^32 枚举不可行，但资源消耗面要堵：同 IP 每分钟最多
    // 120 次探测（覆盖正常用户多 passkey 端点的合理使用）。
    let ip = crate::http::client_ip(req);
    let ip_key = format!("rl:{scope}-ip:{}", ip);
    let m: i64 = c.incr(&ip_key, 1).await.map_err(|e| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用: {e}"))
    })?;
    if m == 1 {
        let _: () = c.expire(&ip_key, 60).await.unwrap_or(());
    }
    if m > PASSKEY_IP_PER_MIN {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// passkey 形状校验必须与生成器同口径（32 位小写字母数字）。
    /// 别名路径靠它把畸形输入挡在 Location 头之外。
    #[test]
    fn passkey_shape_is_strict() {
        assert!(valid_passkey("771ffcb485a9482fa16572765d8696da"));
        assert!(valid_passkey(&"a".repeat(32)));
        assert!(valid_passkey(&format!("{}01", "0123456789".repeat(3))));
        // 长度不对
        assert!(!valid_passkey("abc"));
        assert!(!valid_passkey(&"a".repeat(33)));
        // 大写 / 符号 / 空白 / 换行一律拒
        assert!(!valid_passkey(&"A".repeat(32)));
        for tail in ["-", " ", "\n", "\t", "_", "/", "?"] {
            let p = format!("{}{tail}", "a".repeat(31));
            assert!(!valid_passkey(&p), "尾字符 {tail:?} 应被拒");
        }
        assert!(!valid_passkey(""));
    }
}

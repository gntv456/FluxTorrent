//! 兼容层共享工具：凭据哈希与限流桶。

use actix_web::web;
use sha3::{Digest, Sha3_256};

use crate::errors::DomainResult;
use crate::state::AppState;

/// 凭据端点的同 IP 每分钟探测上限（防换凭据绕过分桶的资源消耗面）。
pub(crate) const PASSKEY_IP_PER_MIN: i64 = 120;

pub(super) fn sha3_hex(bytes: &[u8]) -> String {
    let mut h = Sha3_256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 凭据维度限流：桶 = sha3(凭据) 前 16 位，60s 固定窗口。
/// Redis 故障时 fail-close（与 http::throttle 一致，不给抖动期留缺口）。
pub(crate) async fn limit_passkey(
    state: &web::Data<std::sync::Arc<AppState>>,
    req: &actix_web::HttpRequest,
    scope: &str,
    passkey: &str,
    max: i64,
) -> DomainResult<()> {
    // P-67：两维都交给 `http::rate_window`——它 fail-close，且把窗口写进
    // 键名。旧写法虽然 incr 已 map_err，但 EXPIRE 只在 n==1 时做且吞错：
    // 那条命令一失败就留下无 TTL 的计数键 ⇒ 该 passkey/IP 永久 429。
    let bucket = sha3_hex(passkey.as_bytes());
    // P-72：调用方**不要**再自带 `rl:` 前缀——rate_window 自己加，两处都写
    // 就落出 `rl:rl:npdl:…` 这种双前缀键（实测线上就是这个形状），与运维
    // 文档/闸门脚本约定的 `rl:<scope>:<id>:<窗口>` 对不上。
    crate::http::rate_window(
        state,
        &format!("{scope}:{}", &bucket[..16]),
        max,
        60,
    )
    .await?;
    // IP 第二维：只按被试凭据分桶时，每次换随机值即新桶 = 等效无限流。
    // 同 IP 每分钟最多 120 次探测（覆盖正常多设备使用）。
    let ip = crate::http::client_ip(req);
    crate::http::rate_window(
        state,
        &format!("{scope}-ip:{ip}"),
        PASSKEY_IP_PER_MIN,
        60,
    )
    .await
    .map(|_| ())
}

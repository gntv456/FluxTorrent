//! Passkey（WebAuthn）共享工具：base64url、authData 解析、challenge 存取。
//! 注册仪式在 passkey_reg.rs、登录仪式在 passkey_login.rs（300 行门禁）。

use base64::Engine;
use redis::AsyncCommands;

use crate::state::AppState;
use actix_web::HttpRequest;

pub(super) fn b64url(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .unwrap_or_default()
}

/// authenticatorData 前 32 字节 = rpIdHash
pub(super) fn rp_id_hash(auth_data: &[u8]) -> Option<[u8; 32]> {
    if auth_data.len() < 37 {
        return None;
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&auth_data[..32]);
    Some(h)
}

/// flags 字节（auth_data[32]）的 UV（0x04）位在登录侧消费
pub(super) fn flags(auth_data: &[u8]) -> Option<u8> {
    auth_data.get(32).copied()
}

/// challenge 写入（Redis 5 分钟，与图形验证码同口径）
pub(super) async fn store_challenge(
    state: &AppState,
    key: String,
    val: String,
) {
    let mut c = state.redis.clone();
    let _: () = AsyncCommands::set_ex(&mut c, key, val, 300)
        .await
        .unwrap_or(());
}

/// challenge 原子取删（GETDEL：一次性语义）
pub(super) async fn take_challenge(
    state: &AppState,
    key: String,
) -> Option<String> {
    let mut c = state.redis.clone();
    redis::cmd("GETDEL")
        .arg(&key)
        .query_async(&mut c)
        .await
        .unwrap_or(None)
}

/// RP ID（= 站点注册域，WebAuthn 纪律）：从 announce_url 取 host。
/// 前端必须用同一 rpId，跨子域不匹配会直接验证失败——属于防钓鱼设计。
/// 取不到时回落请求 Host（开发机 localhost 也能走通浏览器注册）。
pub(super) async fn rp_id(state: &AppState, req: &HttpRequest) -> String {
    let announce: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'announce_url'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let from_url = announce.and_then(|u| {
        let rest = u.split_once("://").map(|(_, r)| r).unwrap_or(&u);
        let host = rest.split(['/', ':', '?']).next().unwrap_or("").trim();
        if host.is_empty() {
            None
        } else {
            Some(host.to_string())
        }
    });
    from_url.unwrap_or_else(|| {
        req.headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(|h| h.split(':').next().unwrap_or(h).to_string())
            .unwrap_or_else(|| "localhost".into())
    })
}

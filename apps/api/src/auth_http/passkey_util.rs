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

/// 从 `http://host:port/path` 或裸 `host:port` 里取 host。
fn host_of(raw: &str) -> Option<String> {
    let rest = raw.split_once("://").map(|(_, r)| r).unwrap_or(raw);
    let host = rest.split(['/', ':', '?']).next().unwrap_or("").trim();
    (!host.is_empty()).then(|| host.to_string())
}

/// RP ID（= 用户当前页面的注册域，WebAuthn 纪律）：**站点对外地址**的 host。
///
/// P-114：旧实现第一优先级取 `announce_url` 的 host，注释还把这事说成
/// 「防钓鱼设计」——恰好弄反了。浏览器要求 rpId 与页面所在的可注册域一致，
/// 而 PT 站的 announce 经常在另一台主机上（甚至裸 IP：现网
/// `announce_url=http://127.0.0.1:7070` 而站点是 `http://localhost:3000`），
/// 于是每一次 `navigator.credentials.create` 都在**浏览器里**当场抛
/// SecurityError——服务端连问都没被问到，红线现场就是面板上那句英文报错。
/// announce 与站点同主机的那些部署行为不变；不同主机的，此前根本没人能
/// 用浏览器绑成，所以没有「老凭证突然验不过」的实际受影响面。
///
/// 口径与邮件里的站内链接同源（`PUBLIC_SITE_URL`，回落 `PUBLIC_WEB_URL`），
/// 都没配时退到浏览器自己报的 `Origin`，再退 `Host`，最后 localhost。
pub(super) fn rp_id(req: &HttpRequest) -> String {
    for key in ["PUBLIC_SITE_URL", "PUBLIC_WEB_URL"] {
        if let Some(h) = host_of(&std::env::var(key).unwrap_or_default()) {
            return h;
        }
    }
    let hdr = req.headers();
    for name in ["origin", "host"] {
        if let Some(h) = hdr
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(host_of)
        {
            return h;
        }
    }
    "localhost".into()
}

/// 来源归一：`scheme://authority`，协议与主机小写、去路径与尾斜杠。
/// 非 http(s)（含 `null`）一律归空——沙箱 iframe 与自定义协议都不该
/// 出现在 passkey 的 clientDataJSON 里。
fn normalize_origin(raw: &str) -> String {
    let s = raw.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return String::new();
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return String::new();
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return String::new();
    }
    format!("{}://{}", scheme, authority.to_ascii_lowercase())
}

/// WebAuthn L2 要求 RP 校验 `clientDataJSON.origin`：断言必须由**自家
/// 页面**发起。rpId 只保证「同一注册域」，同域下的兄弟子域（用户空间、
/// 被接管的 CDN 子域）拿到的断言也带合法签名——不看 origin 就等于把
/// passkey 借给整个注册域用。
///
/// 信任集合 = PUBLIC_SITE_URL / PUBLIC_WEB_URL / CORS_ORIGINS（运营方
/// 声明的自家前端来源）；都没配时退回请求同源判定（本机开发）。
pub(super) fn origin_allowed(req: &HttpRequest, raw: &str) -> bool {
    let origin = normalize_origin(raw);
    if origin.is_empty() {
        return false;
    }
    let mut configured: Vec<String> = ["PUBLIC_SITE_URL", "PUBLIC_WEB_URL"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .collect();
    configured.extend(
        std::env::var("CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::to_string),
    );
    let allowed: Vec<String> = configured
        .iter()
        .map(|o| normalize_origin(o))
        .filter(|o| !o.is_empty())
        .collect();
    if !allowed.is_empty() {
        return allowed.iter().any(|o| o == &origin);
    }
    super::origin_matches(req, &origin)
}

//! M27 开放 API：API Token 签发/校验 + OpenAPI 文档 + 开放接口独立限流。
//!
//! 与 JWT 会话分离（方案 M27）：Token 长期有效、sha3-256 哈希落库（明文仅签发时返回一次）、
//! 可独立撤销、独立限流（按 token 维度，默认 60 req/min）。
//! 鉴权头：`Authorization: Token <fxo_...>`（与会话 Bearer 区分）。

mod detail_open;
mod me;
mod me_torrents;
mod open;
mod spec;
mod spec_np;
mod spec_schemas;
mod spec_util;
mod tokens;
mod upload;

use actix_web::{web, HttpRequest, HttpResponse};
use sha3::{Digest, Sha3_256};

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use tokens::{token_issue, token_list, token_refresh, token_revoke};

pub fn mount_openapi(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(open::openapi_spec)
        .service(token_list)
        .service(token_issue)
        .service(token_revoke)
        .service(token_refresh)
        .service(open::open_recent_torrents)
        .service(open::open_announces)
        .service(open::open_categories)
        .service(detail_open::open_torrent_detail)
        .service(me::me_overview)
        .service(me::me_hr)
        .service(me::me_messages)
        .service(me_torrents::me_seeding)
        .service(me_torrents::me_history)
        .service(upload::open_upload)
}

pub(super) fn hash_token(token: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(token.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 开放 API Token 鉴权上下文（0267）：除用户与配额外，附带本窗口已用次数，
/// 供端点回写 `X-RateLimit-*` 头 —— 第三方客户端据此自适应退让，
/// 而不是撞上 429 才知道要降速。
#[derive(Debug, Clone)]
pub struct TokenCtx {
    pub uid: i64,
    pub rpm: i32,
    /// 当前 60s 窗口内已用次数
    pub used: i64,
    /// 当前窗口剩余秒数（固定窗口按分钟边界复位）
    pub reset: i64,
    /// 签发时声明的权限面（0267 起**真正强制**）。
    /// 此前 scopes 只落库不校验 —— 一枚声明 `["read"]` 的 Token
    /// 也能调用后来新增的写端点，等于「用户授权得比他想给的更多」。
    pub scopes: Vec<String>,
}

/// 允许的 scope 白名单（拼错即拒绝，不做静默忽略）
/// API Token 有效期（0069）：签发与滚动续期共用同一 TTL，响应字段同源。
pub const API_TOKEN_TTL_DAYS: i64 = 180;

pub const SCOPE_READ: &str = "read";
pub const SCOPE_UPLOAD: &str = "upload";

impl TokenCtx {
    pub fn remaining(&self) -> i64 {
        (i64::from(self.rpm) - self.used).max(0)
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// 要求 Token 具备某个 scope，否则 403。
/// 写端点（发种）必须显式声明 `upload` —— 用户签 read-only Token 给自动化工具时，
/// 不该因为工具被攻破就带上发种能力。
pub fn require_scope(tk: &TokenCtx, scope: &str) -> DomainResult<()> {
    if tk.has_scope(scope) {
        Ok(())
    } else {
        Err(DomainError::Forbidden)
    }
}

/// 把限流状态挂到既有响应上（不动 body/状态码）。
pub fn with_rl(mut resp: HttpResponse, tk: &TokenCtx) -> HttpResponse {
    use actix_web::http::header::{HeaderName, HeaderValue};
    let h = resp.headers_mut();
    let mut put = |name: &'static str, v: String| {
        if let Ok(val) = HeaderValue::from_str(&v) {
            h.insert(HeaderName::from_static(name), val);
        }
    };
    put("x-ratelimit-limit", tk.rpm.to_string());
    put("x-ratelimit-remaining", tk.remaining().to_string());
    put("x-ratelimit-reset", tk.reset.max(0).to_string());
    resp
}

/// 凭据类响应禁止中间层缓存（0267）。
/// `.torrent` 内嵌本人 passkey、RSS 的 enclosure / 用户信息里也带 passkey ——
/// 一旦被 CDN/反代缓存，就可能把甲的 passkey 发给乙。
/// URL 虽然逐人不同（passkey 在路径/查询里），但「不缓存」是零成本的确定性防线。
pub fn no_store(mut resp: HttpResponse) -> HttpResponse {
    resp.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static(
            "private, no-store, max-age=0",
        ),
    );
    resp
}

/// 开放 API Token 鉴权：返回调用上下文。撤销/不存在 → Unauthorized，
/// 超配额 → RateLimited（429 + Retry-After，见 errors.rs）。
/// 审计修复：①支持 `?apikey=` 查询参数（Prowlarr 等 Torznab 客户端的标准传凭方式）；
/// ②JOIN users 拒绝封禁账号的 token（封禁后立即失效，与 JWT 路径同口径）。
pub async fn require_token(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<TokenCtx> {
    // 头部优先；兼容 Torznab 客户端习惯的 apikey 查询参数
    let apikey_from_query = || -> Option<String> {
        req.uri()
            .query()
            .and_then(|qs| {
                qs.split('&').find_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    (k == "apikey").then(|| v.to_string())
                })
            })
            .filter(|v| !v.is_empty())
    };
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Token "))
        .map(|v| v.to_string())
        .or_else(apikey_from_query)
        .ok_or(DomainError::Unauthorized)?;
    let token_hash = hash_token(&token);
    let row: Option<(i64, i32, Vec<String>)> = sqlx::query_as(
        "SELECT t.user_id, t.rate_per_min, t.scopes FROM api_tokens t \
         JOIN users u ON u.id = t.user_id AND u.status < 2 \
         WHERE t.token_hash = $1 AND t.revoked_at IS NULL \
           AND (t.expires_at IS NULL OR t.expires_at > now()) \
           AND t.created_at > to_timestamp(COALESCE( \
                 (SELECT r.nbf FROM token_revocations r \
                  WHERE r.user_id = t.user_id), 0))",
    )
    .bind(&token_hash)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 无论是否命中都不区分报错（避免枚举探测）
    let Some((uid, rpm, scopes)) = row else {
        return Err(DomainError::Unauthorized);
    };
    // 独立限流：按 token 哈希前 16 位分桶
    let bucket = &token_hash[..16];
    // P-67：旧写法 `unwrap_or(0)` 在 Redis 抖动时既不限流、又把 n=0 填进
    // 下面 TokenCtx.used ⇒ 响应头谎报「还剩满额」，要改 fail-close。
    // P-71：但 P-67 第一版选的 `counter_bump`（每次 EXPIRE 续 60）是
    // **空闲超时窗**不是限流窗——客户端只要不停调用，计数就永不复位，
    // 一枚正常轮询的 Token 累计到 rpm 后会被永久 429。限流必须用
    // 窗口写进键名的固定窗口 `rate_window`，顺带用它的返回值填 used。
    let bucket_key = format!("openapi:{bucket}");
    let n: i64 = crate::http::rate_window(
        &state,
        &bucket_key,
        i64::from(rpm),
        60,
    )
    .await?;
    // 异步刷新 last_used_at（失败不影响请求）
    // 0267 修：此前只绑了 $1、漏绑 $2，sqlx 报参数不足被 `let _ =` 吞掉
    // → last_used_at 永远不更新（工具面板看不出 Token 何时被用过）。
    let db = state.repo.db.clone();
    let hash2 = token_hash.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE api_tokens SET last_used_at = now() WHERE \
             user_id = $1 AND token_hash = $2",
        )
        .bind(uid)
        .bind(&hash2)
        .execute(&db)
        .await;
    });
    Ok(TokenCtx {
        uid,
        rpm,
        used: n,
        // P-71：窗口进了键名 ⇒ 复位时刻就是分钟边界，报「距边界几秒」；
        // 此前恒 60 会让退让的客户端白等（边界可能 3 秒后就到）。
        reset: 60 - chrono::Utc::now().timestamp() % 60,
        scopes,
    })
}

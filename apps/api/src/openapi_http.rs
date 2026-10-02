//! M27 开放 API：API Token 签发/校验 + OpenAPI 文档 + 开放接口独立限流。
//!
//! 与 JWT 会话分离（方案 M27）：Token 长期有效、sha3-256 哈希落库（明文仅签发时返回一次）、
//! 可独立撤销、独立限流（按 token 维度，默认 60 req/min）。
//! 鉴权头：`Authorization: Token <fxo_...>`（与会话 Bearer 区分）。

mod open;
mod upload;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use redis::AsyncCommands;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_openapi(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(open::openapi_spec)
        .service(token_list)
        .service(token_issue)
        .service(token_revoke)
        .service(token_refresh)
        .service(open::open_recent_torrents)
        .service(open::open_announces)
        .service(upload::open_upload)
}

fn hash_token(token: &str) -> String {
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
    /// 当前窗口剩余秒数（Redis TTL；拿不到时回落 60）
    pub reset: i64,
    /// 签发时声明的权限面（0267 起**真正强制**）。
    /// 此前 scopes 只落库不校验 —— 一枚声明 `["read"]` 的 Token
    /// 也能调用后来新增的写端点，等于「用户授权得比他想给的更多」。
    pub scopes: Vec<String>,
}

/// 允许的 scope 白名单（拼错即拒绝，不做静默忽略）
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
           AND (t.expires_at IS NULL OR t.expires_at > now())",
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
    let key = format!("rl:openapi:{bucket}");
    let mut c = state.redis.clone();
    let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = c.expire(&key, 60).await.unwrap_or(());
    }
    if n > i64::from(rpm) {
        return Err(DomainError::RateLimited);
    }
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
        reset: 60,
        scopes,
    })
}

// ============ Token 管理（用户会话鉴权） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TokenRow {
    id: i64,
    name: String,
    scopes: Vec<String>,
    rate_per_min: i32,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/tokens")]
async fn token_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<TokenRow> = sqlx::query_as(
        "SELECT id, name, scopes, rate_per_min, last_used_at, expires_at, revoked_at, created_at \
         FROM api_tokens WHERE user_id = $1 ORDER BY id DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct IssueTokenReq {
    name: String,
    #[serde(default = "default_scopes")]
    scopes: Vec<String>,
    #[serde(default = "default_rpm")]
    rate_per_min: i32,
}
fn default_scopes() -> Vec<String> {
    vec!["read".into()]
}
fn default_rpm() -> i32 {
    60
}

#[post("/me/tokens")]
async fn token_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IssueTokenReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 50 {
        return Err(DomainError::Validation("name 长度 1-50".into()));
    }
    if !(1..=600).contains(&body.rate_per_min) {
        return Err(DomainError::Validation("rate_per_min 取值 1-600".into()));
    }
    // 0267：scope 白名单校验。写端点（/open/torrents）靠 `upload` 把关，
    // 拼错的 scope 必须报错 —— 静默接受会变成「用户以为给了权限、实际没有」
    // 或更糟的反面（未来扩了白名单就悄悄多授权）。
    for s in &body.scopes {
        if s != SCOPE_READ && s != SCOPE_UPLOAD {
            return Err(DomainError::Validation(format!(
                "未知 scope：{s}（可用：read / upload）"
            )));
        }
    }
    if body.scopes.is_empty() {
        return Err(DomainError::Validation(
            "scopes 不能为空（至少 read）".into(),
        ));
    }
    // read 是所有 Token 的隐含权限（用于列表/搜索/用户信息），显式声明 upload
    // 才额外获得发种能力 —— 避免出现「能发种但看不了列表」这种半残状态。
    let scopes: Vec<String> = {
        let mut v = vec![SCOPE_READ.to_string()];
        for s in &body.scopes {
            if s != SCOPE_READ && !v.iter().any(|x| x == s) {
                v.push(s.clone());
            }
        }
        v
    };
    // 每人最多 3 枚有效 token（YemaPT 口径：少而精，泄露面可控）
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM api_tokens WHERE user_id = $1 AND \
         revoked_at IS NULL",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if active >= 3 {
        return Err(DomainError::Validation(
            "有效 token 上限 3 枚，请先撤销旧的".into(),
        ));
    }
    // 明文仅此一次返回：fxo_ 前缀 + 32 随机字节 hex；有效期 180 天（0069）
    let plain = format!("fxo_{}", uuid::Uuid::new_v4().simple());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO api_tokens (user_id, name, token_hash, scopes, rate_per_min, expires_at) \
         VALUES ($1, $2, $3, $4, $5, now() + interval '180 days') RETURNING id",
    )
    .bind(auth.id)
    .bind(name)
    .bind(hash_token(&plain))
    .bind(&scopes)
    .bind(body.rate_per_min)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "apitoken.issue", Some(id))
        .await;
    Ok(ok(serde_json::json!({
        "id": id, "name": name, "token": plain,
        "scopes": scopes,
        "hint": "明文仅显示一次，请立即保存",
        "usage": "Authorization: Token <fxo_...>",
        "scope_note": "scopes 自 0267 起真正强制：upload 才可调用 POST /open/torrents"
    })))
}

#[derive(Deserialize)]
struct RevokeReq {
    id: i64,
}

#[derive(Deserialize)]
struct RefreshReq {
    id: i64,
}

/// 滚动续期（0267）：把到期时间从「现在」起再推 180 天。
/// NAS 上长期运行的工具（Prowlarr / cross-seed / 自写脚本）此前只能等到
/// 到期后重新签发，而发起端（微信支付/自动化容器）未必有人看着 →
/// Token 静默失效、工具静默停摆。续期不动 Token 明文，只推后到期时间。
#[post("/me/tokens/refresh")]
async fn token_refresh(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RefreshReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let expires: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "UPDATE api_tokens SET expires_at = now() + interval '180 days' \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL \
         RETURNING expires_at",
    )
    .bind(body.id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(expires) = expires else {
        return Err(DomainError::Validation("token 不存在或已撤销".into()));
    };
    state
        .repo
        .audit(Some(auth.id), "apitoken.refresh", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({
        "id": body.id,
        "expires_at": expires,
        "ttl_days": 180,
    })))
}

#[post("/me/tokens/revoke")]
async fn token_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RevokeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE api_tokens SET revoked_at = now() \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(body.id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("token 不存在或已撤销".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "apitoken.revoke", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "revoked": body.id })))
}

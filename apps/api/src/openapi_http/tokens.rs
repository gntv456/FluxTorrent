//! 开放 API Token 的用户侧管理面（列表/签发/续期/撤销，会话鉴权）。
//! 从 openapi_http.rs 按域拆出（300 行门禁）。
//!
//! 这组端点走**会话 JWT**（`require_auth`），不是 Token 自己鉴权——它们
//! 是「给用户发 Token 用」的，不是「给 Token 用」的。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::{hash_token, API_TOKEN_TTL_DAYS, SCOPE_READ, SCOPE_UPLOAD};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{rate_window, require_auth};
use crate::state::AppState;

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

#[utoipa::path(
    get,
    path = "/api/v1/me/tokens",
    tag = "凭据",
    description = "列出我的 Token。",
    responses(
        (status = 200, description = "TokenList（会话鉴权，非 Token 自证）"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/me/tokens")]
pub(super) async fn token_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<TokenRow> = sqlx::query_as(
        "SELECT id, name, scopes, rate_per_min, last_used_at, expires_at, \
         revoked_at, created_at FROM api_tokens WHERE user_id = $1 \
         ORDER BY id DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct IssueTokenReq {
    name: String,
    #[serde(default = "default_scopes")]
    scopes: Vec<String>,
    #[serde(default = "default_rpm")]
    rate_per_min: i32,
    /// P-113：Token 是 180 天的**长期凭证**，且不像会话 JWT 会随改密失效，
    /// 只能逐条撤销 ⇒ 签发必须复核本人（密码，开了 2FA 还要当前码）。
    #[serde(default)]
    password: String,
    #[serde(default)]
    totp: Option<u32>,
}
fn default_scopes() -> Vec<String> {
    vec!["read".into()]
}
fn default_rpm() -> i32 {
    60
}

#[utoipa::path(
    post,
    path = "/api/v1/me/tokens",
    tag = "凭据",
    description = "签发 Token（明文仅返回一次）：180 天、每人 ≤3 枚；\
scopes 白名单 read/upload，read 恒隐含 —— 发种必须显式 upload。",
    params(
        ("name" = String, Query, description = "Token 名称 1-50（body 字段）"),
        ("scopes" = String, Query, description = "read / upload（body 字段）"),
        ("rate_per_min" = String, Query, description = "1-600，默认 60（body 字段）"),
    ),
    responses(
        (status = 200, description = "TokenIssued（含一次性明文 token）"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 429, description = "复核限流 5 次/分钟（code 1015）"),
    )
)]
#[post("/me/tokens")]
pub(super) async fn token_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IssueTokenReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 先上档再复核：argon2 校验是 CPU 活，这一口此前**连限流都没有**，
    // 等于给「拿会话试密码」开门（红线：连发 6 次错密码全被放行）。
    rate_window(&state, &format!("tokissue:{}", auth.id), 5, 60).await?;
    crate::twofa_http::step_up(
        &state,
        auth.id,
        &body.password,
        body.totp,
        "签发 API Token",
    )
    .await?;
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
    // 每人最多 3 枚有效 token（同类站 口径：少而精，泄露面可控）
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
        "INSERT INTO api_tokens \
           (user_id, name, token_hash, scopes, rate_per_min, expires_at) \
         VALUES ($1, $2, $3, $4, $5, \
                 now() + make_interval(days => $6::int)) RETURNING id",
    )
    .bind(auth.id)
    .bind(name)
    .bind(hash_token(&plain))
    .bind(&scopes)
    .bind(body.rate_per_min)
    .bind(API_TOKEN_TTL_DAYS)
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
        "scope_note": "scopes 自 0267 起真正强制：upload 才可调用 \
                       POST /open/torrents"
    })))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct RevokeReq {
    id: i64,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct RefreshReq {
    id: i64,
    /// P-113：续期等于「把这枚长期凭证再往后推 180 天」，与签发同类 ⇒ 同判据
    #[serde(default)]
    password: String,
    #[serde(default)]
    totp: Option<u32>,
}

/// 滚动续期（0267）：把到期时间从「现在」起再推 180 天。
/// NAS 上长期运行的工具（Prowlarr / cross-seed / 自写脚本）此前只能等到
/// 到期后重新签发，而发起端（微信支付/自动化容器）未必有人看着 →
/// Token 静默失效、工具静默停摆。续期不动 Token 明文，只推后到期时间。
#[utoipa::path(
    post,
    path = "/api/v1/me/tokens/refresh",
    tag = "凭据",
    description = "滚动续期 180 天（不动明文，只推后到期时间）。",
    responses(
        (status = 200, description = "id + expires_at + ttl_days"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 429, description = "复核限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/me/tokens/refresh")]
pub(super) async fn token_refresh(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RefreshReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    rate_window(&state, &format!("tokrefresh:{}", auth.id), 10, 60).await?;
    crate::twofa_http::step_up(
        &state,
        auth.id,
        &body.password,
        body.totp,
        "续期 API Token",
    )
    .await?;
    let expires: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "UPDATE api_tokens \
           SET expires_at = now() + make_interval(days => $3::int) \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL \
         RETURNING expires_at",
    )
    .bind(body.id)
    .bind(auth.id)
    .bind(API_TOKEN_TTL_DAYS)
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
        "ttl_days": API_TOKEN_TTL_DAYS,
    })))
}

/// 撤销**不**要求复核凭证：这是受害者的自救口——被偷了会话的人第一反应
/// 就是来撤 Token，再加一道密码反而挡住自救（与 passkey 解绑同口径）。
#[utoipa::path(
    post,
    path = "/api/v1/me/tokens/revoke",
    tag = "凭据",
    description = "吊销 Token（受害者自救口：不要求复核凭证）。",
    responses(
        (status = 200, description = "revoked: <id>"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[post("/me/tokens/revoke")]
pub(super) async fn token_revoke(
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

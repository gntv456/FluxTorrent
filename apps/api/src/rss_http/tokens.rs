//! RSS token 管理（0352）：可命名/可轮换/可撤销的订阅凭证。
//! 动机：passkey 烤进每个 .torrent 的 announce 与订阅 URL，rotate 等于全量
//! 换种；命名 token 泄露只需单独撤销，与主 passkey 解耦。
//! 口径：明文 `fxr_` + 32 位随机（OsRng），落库仅 sha3-256 哈希，明文只在
//! 创建/轮换响应出现一次。`/rss/{passkey}` 旧 URL 永远兼容（先查本表再
//! 回落）。上限 5 个有效 token 走应用层校验（产品策略不锁进 schema）。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::openapi_http::no_store;
use crate::state::AppState;

/// 每人有效 token 上限（0352 迁移注释同源：应用层校验，不进 schema）
const RSS_TOKEN_MAX: i64 = 5;

fn sha3_hex(bytes: &[u8]) -> String {
    let mut h = Sha3_256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 生成 `fxr_` + 32 位小写字母数字（对外凭据用 crypto RNG）。
fn new_token_plain() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rngs::OsRng;
    let body: String = (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    format!("fxr_{body}")
}

/// URL 路径形状：fxr_ + 32 位小写字母数字（passkey 是无前缀 32 位）。
pub(crate) fn valid_token_shape(t: &str) -> bool {
    let body = t.strip_prefix("fxr_").unwrap_or_default();
    body.len() == 32
        && body
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// 写限流（创建/轮换共用一桶，每用户 10 次/分钟）。
async fn limit_write(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<()> {
    crate::http::rate_window(state, &format!("rsstok:write:{uid}"), 10, 60)
        .await
        .map(|_| ())
}

#[derive(Deserialize)]
struct CreateReq {
    name: String,
}
#[derive(sqlx::FromRow)]
struct TokenRow {
    id: i64,
    name: String,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

/// 创建（计数上限 + 落库）返回 (行, 明文)：INSERT 自带 COUNT 谓词防并发超限。
async fn insert_token(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    name: &str,
) -> DomainResult<(TokenRow, String)> {
    let plain = new_token_plain();
    let row: TokenRow = sqlx::query_as(
        "INSERT INTO rss_tokens (user_id, name, token_hash) \
         SELECT $1, $2, $3 \
         WHERE (SELECT count(*) FROM rss_tokens \
                WHERE user_id = $1 AND revoked_at IS NULL) < $4 \
         RETURNING id, name, created_at, last_used_at",
    )
    .bind(uid)
    .bind(name)
    .bind(sha3_hex(plain.as_bytes()))
    .bind(RSS_TOKEN_MAX)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or_else(|| {
        DomainError::Validation(format!(
            "有效 RSS token 已达上限 {RSS_TOKEN_MAX} 个，请先撤销"
        ))
    })?;
    Ok((row, plain))
}

#[utoipa::path(
    get,
    path = "/api/v1/rss/tokens",
    tag = "RSS Token",
    description = "列出我的有效 RSS token 元数据（不回明文，库里只有哈希）。",
    responses(
        (status = 200, description = "RssTokenList：tokens[] + limit"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/rss/tokens")]
pub(crate) async fn rss_token_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<TokenRow> = sqlx::query_as(
        "SELECT id, name, created_at, last_used_at FROM rss_tokens \
         WHERE user_id = $1 AND revoked_at IS NULL \
         ORDER BY created_at DESC, id DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({ "id": r.id, "name": r.name,
                "created_at": r.created_at,
                "last_used_at": r.last_used_at })
        })
        .collect();
    Ok(ok(serde_json::json!(
        { "tokens": items, "limit": RSS_TOKEN_MAX }
    )))
}

/// utoipa request_body 引用的 schema（生成器按名取 components）。
/// 与 CreateReq 双名并存：避免 serde 结构体同时当文档 schema 的可见性扩散。
#[derive(utoipa::ToSchema)]
#[schema(as = RssTokenCreateReq)]
pub(crate) struct RssTokenCreateReq {
    /// Token 名称（1-40 字符，文档字段）
    #[allow(dead_code)]
    pub(crate) name: String,
}

/// POST /rss/tokens：创建命名 token；明文仅本次响应可见（no_store）。
#[utoipa::path(
    post,
    path = "/api/v1/rss/tokens",
    tag = "RSS Token",
    description = "创建命名 token（明文仅本次响应可见）；每人有效上限 5 个，\
10 次/分钟限流。",
    request_body = crate::rss_http::tokens::RssTokenCreateReq,
    responses(
        (status = 200, description = "RssTokenCreated：元数据 + 一次性明文 token"),
        (status = 400, description = "名称长度 1-40 / 超上限（code 1002）"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 429, description = "写限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/rss/tokens")]
pub(crate) async fn rss_token_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(DomainError::Validation(
            "token 名称长度需在 1..=40 字符之间".into(),
        ));
    }
    limit_write(&state, auth.id).await?;
    let (row, plain) = insert_token(&state, auth.id, name).await?;
    // 响应里含一次性明文：禁止中间层缓存
    Ok(no_store(ok(
        serde_json::json!({ "id": row.id, "token": plain,
        "name": row.name, "created_at": row.created_at,
        "hint": "明文仅显示一次，请立即复制保存" }),
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/rss/tokens/{id}/rotate",
    tag = "RSS Token",
    description = "轮换：吊销旧 token + 新建同名 token（明文仅本次响应可见）。",
    params(
        ("id" = i64, Path, description = "token id"),
    ),
    responses(
        (status = 200, description = "RssTokenCreated：新元数据 + 一次性明文 token"),
        (status = 404, description = "token 不存在/非本人/已吊销（code 1004）"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 429, description = "写限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/rss/tokens/{id}/rotate")]
pub(crate) async fn rss_token_rotate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    limit_write(&state, auth.id).await?;
    let name: Option<String> = sqlx::query_scalar(
        "UPDATE rss_tokens SET revoked_at = now() \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL \
         RETURNING name",
    )
    .bind(id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 越权口径：id 存在但不属于本人/已吊销 → NotFound（不泄露存在性）
    let Some(name) = name else {
        return Err(DomainError::NotFound(id));
    };
    let (row, plain) = insert_token(&state, auth.id, &name).await?;
    Ok(no_store(ok(
        serde_json::json!({ "id": row.id, "token": plain,
        "name": row.name, "created_at": row.created_at,
        "hint": "旧 token 已失效；新明文仅显示一次，请立即保存" }),
    )))
}

#[utoipa::path(
    delete,
    path = "/api/v1/rss/tokens/{id}",
    tag = "RSS Token",
    description = "软删 token（revoked_at=now()，历史行留审计）。",
    params(
        ("id" = i64, Path, description = "token id"),
    ),
    responses(
        (status = 200, description = "revoked: 1"),
        (status = 404, description = "token 不存在/非本人/已吊销（code 1004）"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[delete("/rss/tokens/{id}")]
pub(crate) async fn rss_token_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE rss_tokens SET revoked_at = now() \
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    Ok(ok(serde_json::json!({ "revoked": n })))
}

/// 挂载 token 管理端点（mount_rss 调用；消费点改造见 rss_feed/forum）。
pub fn mount_tokens(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(rss_token_list)
        .service(rss_token_create)
        .service(rss_token_rotate)
        .service(rss_token_revoke)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_shape_matches_generator() {
        let plain = new_token_plain();
        assert!(plain.starts_with("fxr_") && plain.len() == 36);
        assert!(valid_token_shape(&plain));
        // passkey（无前缀 32 位）不是 token——两个凭证体系不互相误吞
        assert!(!valid_token_shape(&"a".repeat(32)));
        assert!(!valid_token_shape("fxr_ABCDEFGH"));
        assert!(!valid_token_shape(&format!("fxr_{}", "a".repeat(31))));
        assert!(!valid_token_shape(""));
    }

    /// 鉴权 SQL 谓词形状锁定（同 download_keys 口径）：撤销必须生效。
    #[test]
    fn auth_predicate_excludes_revoked() {
        let sql = super::super::authz::AUTH_BY_TOKEN_SQL;
        let s = sql.to_lowercase();
        assert!(
            s.contains("revoked_at is null"),
            "token 鉴权必须排除已撤销行: {sql}"
        );
        assert!(
            s.contains("token_hash = $1"),
            "token 鉴权必须按哈希命中: {sql}"
        );
    }
}

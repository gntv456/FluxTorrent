//! RSS token 管理（0352）：可命名/可轮换/可撤销的订阅凭证。
//!
//! 动机：passkey 烤进每个 .torrent 的 announce 与既有订阅 URL，rotate 等于
//! 全量换种；命名 token 泄露只需单独撤销，与主 passkey 解耦。
//! 口径：明文 `fxr_` + 32 位随机（同 new_passkey 的 OsRng 源），落库仅
//! sha3-256 哈希（先例：0069 download_keys），明文只在创建/轮换响应出现一次。
//! 兼容：`/rss/{passkey}` 旧 URL 永远有效——消费点先查本表再回落 passkey。
//! 上限 5 个有效 token 用应用层校验（产品策略不锁进 schema）。

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

/// 生成 `fxr_` + 32 位小写字母数字（随机源与 domain::new_passkey 同口径：
/// 对外凭据用 crypto RNG，不用 thread_rng）。
fn new_token_plain() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rngs::OsRng;
    let body: String = (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    format!("fxr_{body}")
}

/// URL 路径形状：fxr_ + 32 位小写字母数字（消费点早拒非法形状省一次查询）。
/// 独立于 valid_passkey（passkey 是 32 位无前缀，两者形状不同）。
pub(crate) fn valid_token_shape(t: &str) -> bool {
    let Some(body) = t.strip_prefix("fxr_") else {
        return false;
    };
    body.len() == 32
        && body
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// 写操作限流（创建/轮换共用一桶，每用户 10 次/分钟）：防脚本刷表。
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

/// 创建（校验 + 计数上限 + 落库），返回 (行, 明文)。单事务保证
/// 「COUNT 后 INSERT」之间不会挤进并发请求突破 5 个上限。
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
            "有效 RSS token 已达上限 {RSS_TOKEN_MAX} 个，请先撤销不用的"
        ))
    })?;
    Ok((row, plain))
}

/// GET /rss/tokens：列当前用户有效 token（不回明文——库里也没有）。
#[get("/rss/tokens")]
async fn list_tokens(
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
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "created_at": r.created_at,
                "last_used_at": r.last_used_at,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "tokens": items,
        "limit": RSS_TOKEN_MAX,
    })))
}

/// POST /rss/tokens：创建命名 token；明文仅本次响应可见（no_store）。
#[post("/rss/tokens")]
async fn create_token(
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
    Ok(no_store(ok(serde_json::json!({
        "id": row.id,
        "name": row.name,
        "created_at": row.created_at,
        "token": plain,
        "hint": "明文仅显示一次，请立即复制保存",
    }))))
}

/// POST /rss/tokens/{id}/rotate：吊销旧 token + 新建同名 token。
/// 拆两步而非 UPDATE token_hash：哈希唯一约束下原地换哈希与插入无差别，
/// 且「旧吊销 + 新建」的审计轨迹更清晰（revoked_at 保留历史行）。
#[post("/rss/tokens/{id}/rotate")]
async fn rotate_token(
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
    // 非 404 语义的越权口径：id 存在但不属于本人/已吊销 → NotFound，
    // 不泄露他人 token id 的存在性
    let Some(name) = name else {
        return Err(DomainError::NotFound(id));
    };
    let (row, plain) = insert_token(&state, auth.id, &name).await?;
    Ok(no_store(ok(serde_json::json!({
        "id": row.id,
        "name": row.name,
        "created_at": row.created_at,
        "token": plain,
        "hint": "旧 token 已失效；新明文仅显示一次，请立即复制保存",
    }))))
}

/// DELETE /rss/tokens/{id}：软删（revoked_at=now()），历史行留审计。
#[delete("/rss/tokens/{id}")]
async fn revoke_token(
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
        .service(list_tokens)
        .service(create_token)
        .service(rotate_token)
        .service(revoke_token)
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

    /// 消费点鉴权 SQL 与吊销/过期谓词的形状锁定（同 download_keys 口径）：
    /// revoked_at IS NULL 必须存在，否则撤销不生效。
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

//! M27 开放 API：API Token 签发/校验 + OpenAPI 文档 + 开放接口独立限流。
//!
//! 与 JWT 会话分离（方案 M27）：Token 长期有效、sha3-256 哈希落库（明文仅签发时返回一次）、
//! 可独立撤销、独立限流（按 token 维度，默认 60 req/min）。
//! 鉴权头：`Authorization: Token <fxo_...>`（与会话 Bearer 区分）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use redis::AsyncCommands;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_openapi(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(openapi_spec)
        .service(token_list)
        .service(token_issue)
        .service(token_revoke)
        .service(open_recent_torrents)
}

fn hash_token(token: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(token.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 开放 API Token 鉴权：返回 (user_id, rate_per_min)。撤销/不存在 → Unauthorized。
pub async fn require_token(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<(i64, i32)> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Token "))
        .ok_or(DomainError::Unauthorized)?;
    let row: Option<(i64, i32)> = sqlx::query_as(
        "SELECT user_id, rate_per_min FROM api_tokens \
         WHERE token_hash = $1 AND revoked_at IS NULL \
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 无论是否命中都不区分报错（避免枚举探测）
    let Some((uid, rpm)) = row else {
        return Err(DomainError::Unauthorized);
    };
    // 独立限流：按 token 哈希前 16 位分桶
    let bucket = &hash_token(token)[..16];
    let key = format!("rl:openapi:{bucket}");
    let mut c = state.redis.clone();
    let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = c.expire(&key, 60).await.unwrap_or(());
    }
    if n > rpm as i64 {
        return Err(DomainError::RateLimited);
    }
    // 异步刷新 last_used_at（失败不影响请求）
    let db = state.repo.db.clone();
    let uid2 = uid;
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE api_tokens SET last_used_at = now() WHERE user_id = $1 AND token_hash = $2",
        )
        .bind(uid2)
        .execute(&db)
        .await;
    });
    Ok((uid, rpm))
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
    // 每人最多 3 枚有效 token（YemaPT 口径：少而精，泄露面可控）
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM api_tokens WHERE user_id = $1 AND revoked_at IS NULL",
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
    .bind(&body.scopes)
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
        "hint": "明文仅显示一次，请立即保存",
        "usage": "Authorization: Token <fxo_...>"
    })))
}

#[derive(Deserialize)]
struct RevokeReq {
    id: i64,
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

// ============ 开放数据接口（Token 鉴权） ============

/// 开放接口示例：最新种子（RSS 增强客户端/移动壳可用的稳定只读契约）
#[get("/open/recent")]
async fn open_recent_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    let rows: Vec<(i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT id, name, small_descr, size, created_epoch \
         FROM (SELECT id, name, small_descr, size, EXTRACT(EPOCH FROM created_at)::bigint AS created_epoch \
               FROM torrents WHERE approval_status = 1 ORDER BY id DESC LIMIT 50) t",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, name, descr, size, ts)| {
            serde_json::json!({
                "id": id, "name": name, "description": descr,
                "size": size, "published_at": ts,
            })
        })
        .collect();
    Ok(ok(items))
}

// ============ OpenAPI 文档（与实现同步维护） ============

#[get("/openapi.json")]
async fn openapi_spec() -> impl Responder {
    ok(serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "FluxTorrent Open API",
            "version": "1.0.0",
            "description": "第三方只读接口（M27）。鉴权：Authorization: Token <fxo_...>。Token 在网页端「我的 → API Token」签发。"
        },
        "servers": [{ "url": "/api/v1" }],
        "components": {
            "securitySchemes": {
                "apiToken": { "type": "apiKey", "in": "header", "name": "Authorization", "description": "值形如 `Token fxo_xxx`" }
            },
            "schemas": {
                "Envelope": {
                    "type": "object",
                    "properties": {
                        "code": { "type": "integer" },
                        "message": { "type": "string" },
                        "data": {},
                        "request_id": { "type": "string", "format": "uuid" }
                    }
                },
                "TorrentSummary": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer", "format": "int64" },
                        "name": { "type": "string" },
                        "description": { "type": "string", "nullable": true },
                        "size": { "type": "integer", "format": "int64" },
                        "published_at": { "type": "integer", "format": "int64", "description": "Unix 秒" }
                    }
                }
            }
        },
        "security": [{ "apiToken": [] }],
        "paths": {
            "/open/recent": {
                "get": {
                    "summary": "最新 50 枚种子",
                    "description": "按 id 倒序的种子摘要，适配 RSS 增强客户端与移动壳。",
                    "responses": {
                        "200": {
                            "description": "Envelope<TorrentSummary[]>",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Envelope" } } }
                        },
                        "401": { "description": "token 无效或已撤销（code 2001）" },
                        "429": { "description": "超出 token 独立限流（code 1015）" }
                    }
                }
            },
            "/openapi.json": {
                "get": {
                    "summary": "本文档",
                    "security": [],
                    "responses": { "200": { "description": "OpenAPI 3.1 文档" } }
                }
            }
        }
    }))
}

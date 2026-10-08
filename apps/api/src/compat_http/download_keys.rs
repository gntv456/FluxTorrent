//! 生态兼容层 ③b：30 分钟临时下载凭证（0069，YemaPT generateDownloadKey 口径）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::common::sha3_hex;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::{no_store, require_token, with_rl};
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents::charge_for_download;

/// 临时下载凭证有效期（0069，YemaPT generateDownloadKey 口径）：
/// SQL 过期窗口与响应 `ttl_minutes` 字段共用同一来源。
const DOWNLOAD_KEY_TTL_MINUTES: i64 = 30;

/// 第三方不能拿长期 API Token 直接下载：先为单个种子签发 30 分钟临时凭证，
/// 再用凭证换 .torrent。Token 泄露的损失窗口从"永久"收敛到"30 分钟"。
#[derive(Deserialize)]
struct DownloadKeyReq {
    torrent_id: i64,
}

#[post("/downloads/keys")]
async fn download_key_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DownloadKeyReq>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let uid = tk.uid;
    let approved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents \
         WHERE id = $1 AND approval_status = 1)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !approved {
        return Err(DomainError::NotFound(body.torrent_id));
    }
    // 签发限流：每用户 10 次/分钟（防刷表）
    {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let key = format!("rl:dlkey:issue:{uid}");
        let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () = c.expire(&key, 60).await.unwrap_or(());
        }
        if n > 10 {
            return Err(DomainError::RateLimited);
        }
    }
    let plain = format!("fxk_{}", uuid::Uuid::new_v4().simple());
    let expires: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "INSERT INTO download_keys (token_hash, user_id, torrent_id, \
         expires_at) \
         VALUES ($1, $2, $3, now() + make_interval(mins => $4)) \
         RETURNING expires_at",
    )
    .bind(sha3_hex(plain.as_bytes()))
    .bind(uid)
    .bind(body.torrent_id)
    .bind(DOWNLOAD_KEY_TTL_MINUTES)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 响应里含一次性凭证明文：禁止中间层缓存
    Ok(no_store(with_rl(
        ok(serde_json::json!({
            "key": plain,
            "expires_at": expires,
            "ttl_minutes": DOWNLOAD_KEY_TTL_MINUTES,
            "download_url": format!(
                          "/api/v1/downloads/{}?token={}",
                          body.torrent_id, plain
                      ),
            "hint": "凭证与用户+种子绑定；明文仅显示一次",
        })),
        &tk,
    )))
}

#[derive(Deserialize)]
struct DlTokenQuery {
    token: String,
}

/// 凭证换 .torrent：无需 Authorization（凭证即凭据），校验哈希/种子/有效期。
/// 与长期 Token / passkey 完全解耦 —— 泄露只影响单个种子 30 分钟。
#[get("/downloads/{torrent_id}")]
async fn download_key_fetch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<DlTokenQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let torrent_id = path.into_inner();
    let hash = sha3_hex(q.token.as_bytes());
    // 凭证级限流：每凭证 20 次/分钟 + IP 级 30 次/分钟（四审 L1：凭证哈希
    // 维度可被未知凭证的盲刷绕过——不存在的哈希每次都换新键，计数恒 1；
    // IP 维度兜住枚举/盲刷面）
    {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let key = format!("rl:dlkey:use:{}", &hash[..16]);
        let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () = c.expire(&key, 60).await.unwrap_or(());
        }
        if n > 20 {
            return Err(DomainError::RateLimited.into());
        }
        let ip = req
            .connection_info()
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let ikey = format!("rl:dlkey:use-ip:{}", ip);
        let ipn: i64 = c.incr(&ikey, 1).await.unwrap_or(0);
        if ipn == 1 {
            let _: () = c.expire(&ikey, 60).await.unwrap_or(());
        }
        if ipn > 30 {
            return Err(DomainError::RateLimited.into());
        }
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT user_id FROM download_keys \
         WHERE token_hash = $1 AND torrent_id = $2 AND expires_at > now()",
    )
    .bind(&hash)
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((user_id,)) = row else {
        return Err(DomainError::Unauthorized.into());
    };
    // 首次使用打点（窗口内可复用，不做一次性）
    let _ = sqlx::query(
        "UPDATE download_keys SET used_at = COALESCE(used_at, now()) \
         WHERE token_hash = $1",
    )
    .bind(&hash)
    .execute(&state.repo.db)
    .await;
    // 付费种子（0086）与网页下载同口径扣费：凭证下载不得绕过 charge_for_download
    charge_for_download(&state.repo.db, user_id, torrent_id)
        .await
        .map_err(actix_web::Error::from)?;
    let body = build_torrent_bytes(&state, user_id, torrent_id).await?;
    // .torrent 内嵌本人 announce（含 passkey）：禁止中间层缓存
    Ok(crate::openapi_http::no_store(
        HttpResponse::Ok()
            .content_type("application/x-bittorrent")
            .body(body),
    ))
}

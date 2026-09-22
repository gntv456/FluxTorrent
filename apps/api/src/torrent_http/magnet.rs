//! 磁力链接端点（六维强化方案 2.4「五缺口」之磁力）。
//! 有 info_hash + passkey announce 即可拼出 magnet（BEP9 口径）：
//! `magnet:?xt=urn:btih:<info_hash>&dn=<name>&tr=<announce>`。
//! announce 与 .torrent 下载（publish_http/download.rs build_torrent_bytes）
//! 同源：站点设定 announce_url / https_announce_url 优先、PUBLIC_TRACKER_URL
//! 兜底、剥尾部 /announce 双保险——两条路径绝不允许拼出不同 tracker 地址。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// percent-encode query 值（magnet 的 dn=/tr= 参数）。
/// RFC 3986 未保留字符直出，其余 UTF-8 字节 %XX（中文名/带符号 URL 均安全）。
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 解析 announce 基址：与 build_torrent_bytes 完全同口径（含剥 /announce 尾）。
async fn announce_base(db: &sqlx::PgPool) -> String {
    async fn setting(db: &sqlx::PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
    }
    let strip_announce = |v: String| -> String {
        match v.strip_suffix("/announce") {
            Some(s) => s.to_string(),
            None => v,
        }
    };
    let env_host = std::env::var("PUBLIC_TRACKER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7070".into())
        .trim_end_matches('/')
        .to_string();
    let base_http =
        strip_announce(setting(db, "announce_url").await.unwrap_or(env_host));
    match setting(db, "https_announce_url").await {
        Some(https) if https != base_http => strip_announce(https),
        _ => base_http,
    }
}

/// GET /torrents/{id}/magnet：返回可直接进客户端的磁力链接。
/// 鉴权与可见性口径同 detail 端点（staff/本人放宽到暂缓/待审）。
#[get("/torrents/{id}/magnet")]
async fn torrent_magnet(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 只取拼 magnet 所需的最小列；approval_status 同时充当存在性检查
    let row: Option<(String, String, i16)> = sqlx::query_as(
        "SELECT info_hash, name, approval_status FROM torrents \
         WHERE id = $1 AND (approval_status = 1 OR approval_status = 0 \
            OR approval_status = 4)",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((info_hash, name, status)) = row else {
        return Err(DomainError::NotFound(id));
    };
    // 与 detail 同口径：暂缓/待审仅本人 + staff 可拿磁力
    if status != 1 {
        let owner: i64 =
            sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
                .bind(id)
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if owner != auth.id && auth.class_id < 90 {
            return Err(DomainError::NotFound(id));
        }
    }
    let passkey: String =
        sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let announce = format!(
        "{}/announce/{}",
        announce_base(&state.repo.db).await,
        passkey
    );
    let magnet = format!(
        "magnet:?xt=urn:btih:{}&dn={}&tr={}",
        info_hash,
        urlencode(&name),
        urlencode(&announce),
    );
    Ok(ok(serde_json::json!({ "magnet": magnet })))
}

#[cfg(test)]
mod tests {
    use super::urlencode;

    #[test]
    fn urlencode_keeps_unreserved_and_escapes_rest() {
        assert_eq!(urlencode("A-b_.~z9"), "A-b_.~z9");
        // 空格 → %20（不用 +，magnet 参数按 RFC3986）
        assert_eq!(urlencode("a b"), "a%20b");
        // 中文按 UTF-8 字节逐个转义
        assert_eq!(urlencode("物理"), "%E7%89%A9%E7%90%86");
        // URL 自带符号必须转义（否则 tr= 参数被截断）
        assert_eq!(
            urlencode("http://h:7070/announce/pk"),
            "http%3A%2F%2Fh%3A7070%2Fannounce%2Fpk"
        );
    }

    #[test]
    fn urlencode_empty_is_empty() {
        assert_eq!(urlencode(""), "");
    }
}

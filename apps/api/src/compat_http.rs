//! 生态兼容层（0069，对照 _doc/主流PT架构横向对比与借鉴.md v2 §4）：
//!
//! 背景刷流/辅种工具圈把站点分为 `nexusphp | gazellepw | unit3d | tnode | discuz | mtorrent`
//! 六种「架构方言」（ptool 官方 README），自研架构若不暴露兼容形状的 API 即零适配。
//! 本模块做三件事：
//!   ① `/compat/meta`                     —— 架构自描述（适配器作者的接入入口）
//!   ② `/compat/nexusphp/*`               —— NexusPHP 字段口径的只读 JSON + download.php 形状下载
//!   ③ `/downloads/keys` + `/downloads/{id}` —— 30 分钟临时下载凭证（与长期 API Token 解耦）
//!
//! 鉴权：开放 API Token（`Authorization: Token fxo_...`，复用 openapi_http::require_token，
//! 含时效/撤销/独立限流）；download.php 与凭证下载走 passkey / 一次性短 token ——
//! 与 NexusPHP 生态工具的既有习惯一致。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::build_torrent_bytes;
use crate::openapi_http::require_token;
use crate::state::AppState;
use crate::torrents::{list_torrents, TorrentFilter};

pub fn mount_compat(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(compat_meta)
        .service(compat_np_user)
        .service(compat_np_torrents)
        .service(compat_np_torrent_detail)
        .service(compat_np_download)
        .service(ptpp_user_info)
        .service(download_key_issue)
        .service(download_key_fetch)
}

fn sha3_hex(bytes: &[u8]) -> String {
    let mut h = Sha3_256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 架构自描述：第三方适配器（ptool / PT-Plugin-Plus / 脚本作者）的接入入口。
/// 无需鉴权 —— 只包含协议形状，不含任何用户数据。
#[get("/compat/meta")]
async fn compat_meta() -> DomainResult<HttpResponse> {
    Ok(ok(serde_json::json!({
        "name": "FluxTorrent",
        "architecture": "fluxtorrent",
        "compat": { "nexusphp": true },
        "api_root": "/api/v1",
        "auth": {
            "token": "Authorization: Token <fxo_...>（网页端「我的 → API Token」签发，180 天有效）",
            "passkey": "download.php 兼容端点使用 passkey 查询参数（与 NexusPHP 工具习惯一致）"
        },
        "endpoints": {
            "user": "/api/v1/compat/nexusphp/user.json",
            "torrents": "/api/v1/compat/nexusphp/torrents.json?page=1",
            "torrent_detail": "/api/v1/compat/nexusphp/torrent/{id}.json",
            "download": "/api/v1/compat/nexusphp/download.php?id={id}&passkey={passkey}",
            "download_key": "POST /api/v1/downloads/keys {torrent_id} → 30 分钟临时凭证",
            "ptpp_user_info": "/api/v1/plugins/ptppUserInfo（PT-Plugin-Plus 字段口径聚合端点）"
        },
        "compatibility_notes": "字段口径对齐 NexusPHP：seeders/leechers/completed/size(字节)/added(Unix 秒)/category。\
            破坏性变更承诺：旧端点保留 ≥2 个版本周期，先发 deprecation 公告（YemaPT 硬删接口为反面教材）。"
    })))
}

// ============ NexusPHP 兼容端点（Token 鉴权，只读） ============

/// 当前 Token 所属用户（NP userdetails 口径：uploaded/downloaded/seedbonus/passkey）
#[get("/compat/nexusphp/user.json")]
async fn compat_np_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (uid, _rpm) = require_token(&req, &state).await?;
    let row: Option<(i64, String, i64, i64, i64, i32, String)> = sqlx::query_as(
        "SELECT id, username, uploaded, downloaded, COALESCE(spark_balance,0), class_id, passkey \
         FROM users WHERE id = $1 AND status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, username, uploaded, downloaded, seedbonus, class_id, passkey)) = row else {
        return Err(DomainError::Unauthorized);
    };
    let ratio = if downloaded > 0 {
        (uploaded as f64 / downloaded as f64 * 10000.0).round() / 10000.0
    } else {
        -1.0
    };
    Ok(ok(serde_json::json!({
        "id": id,
        "username": username,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seedbonus": seedbonus,
        "class": class_id,
        "ratio": ratio,
        "passkey": passkey, // 本人 Token 才能读取；工具用它拼接 download.php 与 tracker 汇报
    })))
}

#[derive(Deserialize)]
struct NpListQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    pagesize: Option<i64>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    category: Option<i32>,
}

/// 种子列表（NP torrents.php 字段口径，游标分页内部转 page 语义）
#[get("/compat/nexusphp/torrents.json")]
async fn compat_np_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NpListQuery>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    let page = q.page.unwrap_or(1).max(1);
    let pagesize = q.pagesize.unwrap_or(30).clamp(1, 50);
    let filter = TorrentFilter {
        category_id: q.category.map(|v| vec![v]),
        search: q.keyword.clone().filter(|k| !k.trim().is_empty()),
        ..TorrentFilter::default()
    };
    let p = list_torrents(&state.repo.db, &filter, None, page * pagesize).await?;
    let skip = ((page - 1) * pagesize) as usize;
    let items: Vec<_> = p
        .items
        .into_iter()
        .skip(skip)
        .take(pagesize as usize)
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "small_descr": t.small_descr,
                "seeders": t.seeders,
                "leechers": t.leechers,
                "completed": t.times_completed,
                "size": t.size,
                "added": t.created_at.timestamp(),
                "category": t.category_id,
                "medium": t.medium_id,
                "promotion": t.promotion,
                "free": matches!(t.promotion.as_deref(), Some("free") | Some("x2free")),
                "official": t.official_tag,
                "sticky": t.sticky,
            })
        })
        .collect();
    let has_more = p.total_estimate > page * pagesize;
    Ok(ok(serde_json::json!({
        "page": page,
        "page_size": pagesize,
        "total_estimate": p.total_estimate,
        "has_more": has_more,
        "items": items,
    })))
}

/// 种子详情（NP details 口径 + 本站扩展；download 字段为模板，passkey 由工具自行拼接）
#[get("/compat/nexusphp/torrent/{id}.json")]
async fn compat_np_torrent_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(String, Option<String>, i64, i32, i32, i32, i32, Option<i64>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT name, descr, size, seeders, leechers, times_completed, category_id, group_id, created_at \
             FROM torrents WHERE id = $1 AND approval_status = 1",
        )
        .bind(id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, descr, size, seeders, leechers, completed, category, group_id, created)) = row
    else {
        return Err(DomainError::NotFound(id));
    };
    Ok(ok(serde_json::json!({
        "id": id,
        "name": name,
        "descr": descr,
        "size": size,
        "seeders": seeders,
        "leechers": leechers,
        "completed": completed,
        "category": category,
        "group_id": group_id,
        "group_api": "/api/v1/torrents/{id}/group",
        "added": created.timestamp(),
        "download": "/api/v1/compat/nexusphp/download.php?id={id}&passkey=<你的 passkey>",
    })))
}

#[derive(Deserialize)]
struct NpDownloadQuery {
    id: i64,
    passkey: String,
}

/// NP 形状下载端点：`download.php?id={id}&passkey={passkey}`。
/// 与 NexusPHP 工具生态的既有 URL 习惯一致（passkey 查询参数鉴权）。
/// 防护与网页下载同口径：过审校验 + 下载闸门（挂起/停下载拒发）+ passkey 限流。
#[get("/compat/nexusphp/download.php")]
async fn compat_np_download(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NpDownloadQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    // passkey 限流（防枚举/暴力）：每 passkey 30 次/分钟，复用 Redis 简单计数
    let bucket = sha3_hex(q.passkey.as_bytes());
    let key = format!("rl:npdl:{}", &bucket[..16]);
    {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () = c.expire(&key, 60).await.unwrap_or(());
        }
        if n > 30 {
            return Err(DomainError::RateLimited.into());
        }
    }
    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE passkey = $1 AND status < 2 AND download_enabled AND NOT suspended",
    )
    .bind(&q.passkey)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(user_id) = user_id else {
        return Err(DomainError::Unauthorized.into());
    };
    let body = build_torrent_bytes(&state, user_id, q.id).await?;
    Ok(HttpResponse::Ok()
        .content_type("application/x-bittorrent")
        .body(body))
}

// ============ PT-Plugin-Plus 聚合端点（朱雀口径） ============

/// `/api/plugins/ptppUserInfo`：按 PT-Plugin-Plus 的字段命名一次性返回用户面板数据。
/// 这是朱雀（zhuque.in）验证过的「自研站进插件生态的最短路径」——插件的用户信息卡片、
/// 等级/魔力展示、未读提醒全部取自本端点，字段名即插件 schema 的约定。
/// 鉴权走开放 API Token（比朱雀的 cookie+CSRF 更适合纯前端插件外的工具）。
#[get("/plugins/ptppUserInfo")]
async fn ptpp_user_info(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (uid, _rpm) = require_token(&req, &state).await?;
    let row: Option<(
        i64,
        String,
        i64,
        i64,
        i64,
        i64,
        i32,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT u.id, u.username, u.spark_balance, u.uploaded, u.downloaded, u.seeding_size, \
                u.class_id, u.created_at \
         FROM users u WHERE u.id = $1 AND u.status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, name, bonus, uploaded, downloaded, seeding_size, class_id, join_time)) = row
    else {
        return Err(DomainError::Unauthorized);
    };
    let class_name: String = sqlx::query_scalar("SELECT name FROM user_classes WHERE id = $1")
        .bind(class_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .unwrap_or_else(|| "LV0".into());
    let (seeding, leeching, invites, unread_messages): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding), \
            (SELECT count(*) FROM snatches WHERE user_id = $1 AND leeching), \
            (SELECT count(*) FROM invites WHERE inviter_id = $1 AND status = 0 AND expires_at > now()), \
            (SELECT count(*) FROM messages WHERE receiver_id = $1 AND read_at IS NULL)",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        // 插件契约字段（PT-Plugin-Plus TNode schema 口径）
        "id": id,
        "name": name,
        "bonus": bonus,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seeding": seeding,
        "leeching": leeching,
        "seedingSize": seeding_size,
        "invites": invites,
        "levelName": class_name,
        "joinTime": join_time,
        "messageCount": unread_messages,
        "isLogged": true,
    })))
}

// ============ 30 分钟临时下载凭证（0069，YemaPT generateDownloadKey 口径） ============

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
    let (uid, _rpm) = require_token(&req, &state).await?;
    let approved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
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
        "INSERT INTO download_keys (token_hash, user_id, torrent_id, expires_at) \
         VALUES ($1, $2, $3, now() + interval '30 minutes') RETURNING expires_at",
    )
    .bind(sha3_hex(plain.as_bytes()))
    .bind(uid)
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "key": plain,
        "expires_at": expires,
        "ttl_minutes": 30,
        "download_url": format!("/api/v1/downloads/{}?token={}", body.torrent_id, plain),
        "hint": "凭证与用户+种子绑定；明文仅显示一次",
    })))
}

#[derive(Deserialize)]
struct DlTokenQuery {
    token: String,
}

/// 凭证换 .torrent：无需 Authorization（凭证即凭据），校验哈希/种子/有效期。
/// 与长期 Token / passkey 完全解耦 —— 泄露只影响单个种子 30 分钟。
#[get("/downloads/{torrent_id}")]
async fn download_key_fetch(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<DlTokenQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let torrent_id = path.into_inner();
    let hash = sha3_hex(q.token.as_bytes());
    // 凭证级限流：每凭证 20 次/分钟
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
        "UPDATE download_keys SET used_at = COALESCE(used_at, now()) WHERE token_hash = $1",
    )
    .bind(&hash)
    .execute(&state.repo.db)
    .await;
    let body = build_torrent_bytes(&state, user_id, torrent_id).await?;
    Ok(HttpResponse::Ok()
        .content_type("application/x-bittorrent")
        .body(body))
}

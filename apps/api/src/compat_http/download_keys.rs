//! 生态兼容层 ③b：30 分钟临时下载凭证（0069，同类站 generateDownloadKey 口径）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::common::sha3_hex;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::{no_store, require_token, with_rl};
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents::charge_for_download;

/// 临时下载凭证有效期（0069，同类站 generateDownloadKey 口径）：
/// SQL 过期窗口与响应 `ttl_minutes` 共源；C1 会话口 `/torrents/{id}/download-link` 同引用。
pub(crate) const DOWNLOAD_KEY_TTL_MINUTES: i64 = 30;

/// 第三方不能拿长期 API Token 直接下载：先为单个种子签发 30 分钟临时凭证，
/// 再用凭证换 .torrent。Token 泄露的损失窗口从"永久"收敛到"30 分钟"。
#[derive(Deserialize, utoipa::ToSchema)]
struct DownloadKeyReq {
    torrent_id: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/downloads/keys",
    tag = "兼容层(NP)",
    description = "签发 30 分钟临时下载凭证：把「下载」从长期 Token/passkey \
里解耦，泄露窗口 30 分钟；每用户 10 次/分钟。body：{torrent_id}（JSON）。",
    request_body = DownloadKeyReq,
    responses(
        (status = 200, description = "DownloadKey：key + download_url"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 404, description = "种子不存在或未过审（code 1004）"),
        (status = 429, description = "签发限流 10 次/分钟（code 1015）"),
    )
)]
/// 审核检查 + 签发临时下载凭证（审核存在性、INSERT、明文生成）。
/// `download_key_issue` 与 crossseed dl（302 复用下载链路）共用——
/// 辅种工具拿长期 Token 现场换一枚 30 分钟凭证，不放大泄露窗口。
pub(crate) async fn issue_download_key(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    torrent_id: i64,
) -> DomainResult<(String, chrono::DateTime<chrono::Utc>)> {
    let approved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents \
         WHERE id = $1 AND approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !approved {
        return Err(DomainError::NotFound(torrent_id));
    }
    // P-344：可执行凭证不能绕过下载闸门。本函数先后被三个口已经共用（
    // `POST /downloads/keys`、`GET /crossseed/dl/{id}`、NP 兼容层），而它原来只判了
    // approval_status。实测（批一百五十四）：root 未购任何付费种，
    // `POST /downloads/keys` 签出 **fxk_ 凭证** 并真落了一行 download_keys
    // （同一种子 `/torrents/{id}/magnet` 是回 400 并带明确文案的）。
    // 即使不重复扣费，也不能给未购的人发凭证——否则「点一下下载」
    // 就是一条绕过扣费的下载通道。
    // 与 magnet 同口径三道闸：① 下载资格（被停下载/挂起账号照拿）；
    // ② 付费种必须已购（本人发布者免费）；③ 归属不可达。
    crate::torrents::ensure_downloadable(&state.repo.db, uid).await?;
    let paid: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT price, owner_id FROM torrents WHERE id = $1",
    )
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some((price, owner)) = paid {
        if price > 0 && owner != Some(uid) {
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM torrent_purchases \
                 WHERE user_id = $1 AND torrent_id = $2)",
            )
            .bind(uid)
            .bind(torrent_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !owned {
                return Err(DomainError::Validation(format!(
                    "该种子为付费种子（{} 魔力）：请先在详情页点「下载」取得后再用",
                    price
                )));
            }
        }
    }
    let plain = format!("fxk_{}", uuid::Uuid::new_v4().simple());
    let expires: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "INSERT INTO download_keys (token_hash, user_id, torrent_id, \
         expires_at) \
         VALUES ($1, $2, $3, now() + make_interval(mins => $4::int)) \
         RETURNING expires_at",
    )
    .bind(sha3_hex(plain.as_bytes()))
    .bind(uid)
    .bind(torrent_id)
    .bind(DOWNLOAD_KEY_TTL_MINUTES)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok((plain, expires))
}

#[utoipa::path(
    post,
    path = "/api/v1/downloads/keys",
    tag = "兼容层(NP)",
    description = "签发 30 分钟临时下载凭证：把「下载」从长期 Token/passkey \
里解耦，泄露窗口 30 分钟；每用户 10 次/分钟。body：{torrent_id}（JSON）。",
    request_body = DownloadKeyReq,
    responses(
        (status = 200, description = "DownloadKey：key + download_url"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 404, description = "种子不存在或未过审（code 1004）"),
        (status = 429, description = "签发限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/downloads/keys")]
async fn download_key_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DownloadKeyReq>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let uid = tk.uid;
    // 签发限流：每用户 10 次/分钟（防刷表）
    // P-67：旧写法 `incr(...).unwrap_or(0)` 在 Redis 故障时记不上数
    // ⇒ 永不触发限流（可无限量签发凭证）。统一走 rate_window（fail-close，
    // 窗口写在键名里，不会因续期失败留下无 TTL 的键）。
    crate::http::rate_window(&state, &format!("dlkey:issue:{uid}"), 10, 60)
        .await?;
    let (plain, expires) =
        issue_download_key(&state, uid, body.torrent_id).await?;
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

/// 用户自助吊销请求：torrent_id 缺省 = 吊销本人全部**未过期**凭证；
/// 指定 torrent_id = 只吊销该种子的（其余凭证不受影响，刷流工具链
/// 里单种泄露不必伤及全家）。
#[derive(Deserialize, utoipa::ToSchema)]
struct DownloadKeyRevokeReq {
    torrent_id: Option<i64>,
}

/// DELETE 语义的吊销 SQL：`($2::BIGINT IS NULL OR torrent_id=$2)` 让
/// 「全部」与「单个」共用一条语句；`expires_at > now()` 保证已过期行
/// 不被重复计数（过期行本就不可用，无需真删）。
const REVOKE_KEY_SQL: &str = "DELETE FROM download_keys \
     WHERE user_id = $1 \
       AND ($2::BIGINT IS NULL OR torrent_id = $2) \
       AND expires_at > now()";

/// POST /downloads/keys/revoke：用户自助吊销临时下载凭证。
/// 鉴权与签发同口径（require_token）—— 第三方工具只有 Token 没有
/// 会话，泄露的自救动作必须在同一凭证体系内可完成。
#[utoipa::path(
    post,
    path = "/api/v1/downloads/keys/revoke",
    tag = "兼容层(NP)",
    description = "用户自助吊销临时下载凭证：torrent_id 缺省 = 吊销本人全部\
未过期凭证；指定 torrent_id = 只吊销该种子的。body：{torrent_id?}（JSON）。",
    request_body = DownloadKeyRevokeReq,
    responses(
        (status = 200, description = "revoked 数量 + scope（torrent/all）"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 429, description = "吊销限流 10 次/分钟（code 1015）"),
    )
)]
#[post("/downloads/keys/revoke")]
async fn download_key_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<DownloadKeyRevokeReq>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let uid = tk.uid;
    // 吊销限流独立成键（10 次/分钟）：写操作比签发更敏感，且不与
    // 签发共用计数可以让「签发被限」不连带锁死自救路径。
    crate::http::rate_window(&state, &format!("dlkey:revoke:{uid}"), 10, 60)
        .await?;
    let torrent_id = body.and_then(|b| b.torrent_id);
    let n = sqlx::query(REVOKE_KEY_SQL)
        .bind(uid)
        .bind(torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    Ok(no_store(with_rl(
        ok(serde_json::json!({
            "revoked": n,
            "scope": if torrent_id.is_some() { "torrent" } else { "all" },
            "hint": "已过期凭证不参与计数；撤销立即生效",
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
#[utoipa::path(
    get,
    path = "/api/v1/downloads/{torrent_id}",
    tag = "兼容层(NP)",
    description = "凭证换 .torrent（免 Authorization）：凭证与用户+种子绑定；\
每凭证 20 次/分钟 + IP 维 30 次/分钟。",
    params(("token" = String, Query, description = "fxk_ 凭证（必填）")),
    responses(
        (status = 200, description = "application/x-bittorrent"),
        (status = 401, description = "凭证无效/过期（code 2001）"),
        (status = 403, description = "下载闸门拒绝（停下载/挂起/封禁，code 2003）"),
        (status = 404, description = "资源不存在（code 1004）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/downloads/{torrent_id}")]
async fn download_key_fetch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: crate::http::Q<DlTokenQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let torrent_id = path.into_inner();
    let hash = sha3_hex(q.token.as_bytes());
    // 凭证级限流：每凭证 20 次/分钟 + IP 级 30 次/分钟（四审 L1：凭证哈希
    // 维度可被未知凭证的盲刷绕过——不存在的哈希每次都换新键，计数恒 1；
    // IP 维度兜住枚举/盲刷面）
    //
    // P-67：两维都从 `incr(...).unwrap_or(0)` 换成 fail-close 的
    // `rate_window`——旧写法在 Redis 抖动时这两道闸瞬间不存在，而本端点
    // 没有 Authorization，泄露凭证的爆炸半径全靠它们。
    let ip = req
        .connection_info()
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    crate::http::rate_window(
        &state,
        &format!("dlkey:use:{}", &hash[..16]),
        20,
        60,
    )
    .await
    .map_err(actix_web::Error::from)?;
    crate::http::rate_window(&state, &format!("dlkey:use-ip:{ip}"), 30, 60)
        .await
        .map_err(actix_web::Error::from)?;
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
    // 资格闸门先于扣费（P-162）：凭证是「30 分钟内免 Authorization」的
    // 通道，签发之后账号被停下载/挂起/封禁都要当场生效，不能先扣钱再拒发
    crate::torrents::ensure_downloadable(&state.repo.db, user_id)
        .await
        .map_err(actix_web::Error::from)?;
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

#[cfg(test)]
mod tests {
    /// SQL 语义单测：吊销语句必须同时满足「只删自己的」与「只删未过期的」。
    /// 表结构与 REVOKE_KEY_SQL 文本同源断言——本 crate 无 sqlx 测试库，
    /// 用字符串形状锁定关键谓词（user_id 绑定 / torrent_id 可空短路 /
    /// expires_at > now()），SQL 变更时测试随之失败，防止口径漂移。
    #[test]
    fn revoke_sql_scopes_to_owner_unexpired_and_optional_torrent() {
        let s = super::REVOKE_KEY_SQL;
        // 只删本人：user_id 以绑定参数出现，而非字面量（防误写成全局 DELETE）
        assert!(
            s.to_lowercase().contains("where user_id = $1"),
            "revoke SQL 必须以 user_id=$1 限定属主: {s}"
        );
        // 只删未过期：expires_at > now() 必须存在
        assert!(
            s.to_lowercase().contains("expires_at > now()"),
            "revoke SQL 必须排除已过期凭证: {s}"
        );
        // torrent_id 可空短路：缺省吊销全部、指定则只吊销该种子
        assert!(
            s.contains("($2::BIGINT IS NULL OR torrent_id = $2)"),
            "revoke SQL 必须支持 torrent_id 缺省=全部: {s}"
        );
    }
}

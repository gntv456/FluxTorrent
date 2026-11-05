//! RSS / 兼容下载共享鉴权：只认命名 token（fxr_）。
//!
//! passkey 只用于 Tracker 汇报；RSS 与第三方下载口不得再把长期汇报
//! 密钥当通用凭证。token 的撤销与账号状态闸在这里统一收口。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// token 命中查询：按 sha3-256 哈希取有效 token 的 user_id。
/// 撤销闸（revoked_at）与账号状态闸（status < 2）缺一不可——封禁用户
/// 的订阅 token 必须同步失效，与下载口 resolve_user_id 同一把尺。
/// （tokens.rs 单测锁定 `revoked_at IS NULL` 谓词，防止撤销口径漂移。）
pub(crate) const AUTH_BY_TOKEN_SQL: &str =
    "SELECT t.user_id FROM rss_tokens t \
     JOIN users u ON u.id = t.user_id \
     WHERE t.token_hash = $1 AND t.revoked_at IS NULL AND u.status < 2";

/// last_used_at 节流写：仅当距上次打点超过 300s 才 UPDATE——订阅器
/// 分钟级轮询下不打写放大（每次拉取都写一行 UPDATE 毫无价值）。
const TOUCH_THROTTLE_SECS: i64 = 300;

/// RSS 鉴权结果：user_id + 本次使用的 token 明文（enclosure 直链
/// 必须跟随同一枚 token，撤销它即整条订阅链失效）。
pub(crate) struct RssIdentity {
    pub user_id: i64,
    pub class_id: Option<i32>,
    pub credential: String,
}

/// 只按 token 解析；passkey 形状在入口就被拒，不再回落。
/// 返回 None = token 未命中（调用方回 404 unknown）。
pub(crate) async fn resolve_identity(
    state: &web::Data<std::sync::Arc<AppState>>,
    path_cred: &str,
) -> Option<RssIdentity> {
    if !super::tokens::valid_token_shape(path_cred) {
        return None;
    }
    let hash = super::tokens::token_hash_hex(path_cred);
    let uid: Option<i64> = sqlx::query_scalar(AUTH_BY_TOKEN_SQL)
        .bind(&hash)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
    let user_id = uid?;
    touch_last_used(state, user_id, &hash).await;
    Some(RssIdentity {
        user_id,
        class_id: None,
        credential: path_cred.to_string(),
    })
}

/// 下载/兼容口的 token → 用户 id。撤销与账号状态两道闸缺一不可。
pub(crate) async fn resolve_user_id(
    state: &web::Data<std::sync::Arc<AppState>>,
    cred: &str,
) -> DomainResult<Option<i64>> {
    if !super::tokens::valid_token_shape(cred) {
        return Ok(None);
    }
    let hash = super::tokens::token_hash_hex(cred);
    let uid: Option<i64> = sqlx::query_scalar(AUTH_BY_TOKEN_SQL)
        .bind(&hash)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(uid)
}

/// 节流打点 last_used_at（>300s 才写）；写失败只影响展示，不影响鉴权。
async fn touch_last_used(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    hash: &str,
) {
    let _ = sqlx::query(
        "UPDATE rss_tokens SET last_used_at = now() \
         WHERE user_id = $1 AND token_hash = $2 \
           AND (last_used_at IS NULL \
                OR last_used_at < now() - make_interval(secs => $3))",
    )
    .bind(user_id)
    .bind(hash)
    .bind(TOUCH_THROTTLE_SECS)
    .execute(&state.repo.db)
    .await;
}

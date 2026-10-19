//! RSS 端点共享鉴权（0352）：token 优先、passkey 回落的统一解析。
//!
//! 两个消费点（种子 RSS / 论坛 RSS）同款鉴权，抽到 rss_http 根模块，
//! 避免 token→passkey 回落与 last_used_at 节流在两处各自漂移。
//! 旧 passkey URL **永远兼容**：token miss 才回落 user_by_passkey。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// token 命中查询：按 sha3-256 哈希取有效 token 的 user_id。
/// （tokens.rs 单测锁定 `revoked_at IS NULL` 谓词，防止撤销口径漂移。）
pub(crate) const AUTH_BY_TOKEN_SQL: &str = "SELECT user_id FROM rss_tokens \
     WHERE token_hash = $1 AND revoked_at IS NULL";

/// last_used_at 节流写：仅当距上次打点超过 300s 才 UPDATE——订阅器
/// 分钟级轮询下不打写放大（每次拉取都写一行 UPDATE 毫无价值）。
const TOUCH_THROTTLE_SECS: i64 = 300;

/// RSS 鉴权结果：user_id + 鉴权路径里的 passkey/token 明文（后续
/// 拼 enclosure 直链需要——token 消费时种子直链里放的也是该 token，
/// 保持「一个 URL 一个凭证」的自洽）。
pub(crate) struct RssIdentity {
    pub user_id: i64,
    pub class_id: Option<i32>,
    pub credential: String,
}

/// 先按 token（fxr_ 前缀形状早拒非法输入），miss 回落 passkey。
/// 返回 None = 双路径都未命中（调用方回 404 unknown）。
/// DB 抖动按 miss 处理——鉴权失败路径与现状（unwrap_or(None)）一致。
pub(crate) async fn resolve_identity(
    state: &web::Data<std::sync::Arc<AppState>>,
    path_cred: &str,
) -> Option<RssIdentity> {
    if super::tokens::valid_token_shape(path_cred) {
        let hash = super::tokens::token_hash_hex(path_cred);
        let uid: Option<i64> = sqlx::query_scalar(AUTH_BY_TOKEN_SQL)
            .bind(&hash)
            .fetch_optional(&state.repo.db)
            .await
            .unwrap_or(None);
        if let Some(user_id) = uid {
            touch_last_used(state, user_id, &hash).await;
            return Some(RssIdentity {
                user_id,
                class_id: None,
                credential: path_cred.to_string(),
            });
        }
    }
    // 回落：旧 passkey URL（永远兼容）。token 形状不合法的输入走这条
    // 依旧会被 valid_passkey 挡在调用方入口，不额外打 rss_tokens 表。
    let row: Option<(i64, i32)> = sqlx::query_as(
        "SELECT id, class_id FROM user_by_passkey \
         WHERE passkey = $1 AND status < 2",
    )
    .bind(path_cred)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None);
    row.map(|(user_id, class_id)| RssIdentity {
        user_id,
        class_id: Some(class_id),
        credential: path_cred.to_string(),
    })
}

/// 下载/兼容口的凭证 → 用户 id（`fxr_` 命名 token 与主 passkey 两条形状
/// 同一把尺）。RSS 的 `<enclosure>` 直链里放的就是订阅用的那枚 token，
/// `download.php` 若不认它，刷流器照直链一点就 401（P-389 实测：订阅 200、
/// 直链 401 ⇒ token 用户的自动刷流整条断掉）。两条路径都必须过
/// `status < 2`——换凭证不放宽状态闸（P-346 同口径）。
pub(crate) async fn resolve_user_id(
    state: &web::Data<std::sync::Arc<AppState>>,
    cred: &str,
) -> DomainResult<Option<i64>> {
    if super::tokens::valid_token_shape(cred) {
        let hash = super::tokens::token_hash_hex(cred);
        let uid: Option<i64> = sqlx::query_scalar(
            "SELECT t.user_id FROM rss_tokens t \
             JOIN users u ON u.id = t.user_id \
             WHERE t.token_hash = $1 AND t.revoked_at IS NULL AND u.status < 2",
        )
        .bind(&hash)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(uid);
    }
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM user_by_passkey WHERE passkey = $1 AND status < 2",
    )
    .bind(cred)
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

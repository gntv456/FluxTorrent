//! 免费券/中性券（0073 Gazelle FL token 口径）。
//! 从 economy_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 免费券/中性券（0073，Gazelle FL token 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct VoucherRow {
    id: i64,
    kind: String,
    source: String,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_torrent_id: Option<i64>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 我的券库存（含已用/过期历史，前端按状态分组）
#[get("/me/vouchers")]
async fn my_vouchers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<VoucherRow> = sqlx::query_as(
        "SELECT id, kind, source, granted_at, expires_at, used_torrent_id, used_at \
         FROM user_vouchers WHERE user_id = $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 券的真实状态（P-459） ============

/// 为什么这张券用不了——旧写法把三处「SELECT/CAS 没打中」一律回
/// 「券不存在、已使用或已过期」。对**绑定生效中**的券（`used_torrent_id` 已写、
/// `used_at` 仍是 NULL、也还没过期）这三句全是假的，而这恰恰是会员最容易撞上
/// 的状态：`/me/vouchers` 返回 `used_torrent_id`，面板的「可用券」过滤器只看
/// `used_at` 与有效期 ⇒ 绑错种子的券仍被列成可用，点「使用」就被指控用过/过期。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum VoucherState {
    /// 券号不存在，或不属于当前账号
    Missing,
    /// 已核销
    Used,
    /// 过了有效期
    Expired,
    /// 已绑定某张种子，正在等下载量核销
    Bound(i64),
    /// 可用
    Available,
}

/// 状态 → 给用户看的一句话；`Available` 回 None（没有错要说）。
/// 判定与文案分开，是为了让「不许再假指控已使用/已过期」这类断言能单测。
pub(super) fn unusable_reason(s: &VoucherState) -> Option<String> {
    if *s == VoucherState::Available {
        return None;
    }
    Some(match s {
        VoucherState::Missing => "券不存在（或不属于当前账号）".to_string(),
        VoucherState::Used => "该券已核销".to_string(),
        VoucherState::Expired => "该券已过有效期".to_string(),
        VoucherState::Bound(tid) => format!(
            "该券已对种子 #{tid} 生效，下载量超过种子大小 4% 后自动核销；\
             一张券只能绑定一次，如需换绑请联系管理组"
        ),
        VoucherState::Available => return None,
    })
}

/// 查券的真实状态（券号 + 归属一起判，避免把别人的券说成「已过期」）。
pub(super) async fn state_of(
    db: &sqlx::PgPool,
    vid: i64,
    uid: i64,
) -> DomainResult<VoucherState> {
    let row: Option<(
        Option<i64>,
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT used_torrent_id, used_at, expires_at FROM user_vouchers \
         WHERE id = $1 AND user_id = $2",
    )
    .bind(vid)
    .bind(uid)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(match row {
        None => VoucherState::Missing,
        Some((_, Some(_), _)) => VoucherState::Used,
        Some((_, None, exp)) if exp <= chrono::Utc::now() => {
            VoucherState::Expired
        }
        Some((Some(tid), None, _)) => VoucherState::Bound(tid),
        Some((None, None, _)) => VoucherState::Available,
    })
}

/// 给三个拒绝口共用的「到底为什么」；连状态都读不出时退回旧文案，
/// 不把一个 500 递到会员面前。
pub(super) async fn why_unusable(
    db: &sqlx::PgPool,
    vid: i64,
    uid: i64,
) -> String {
    match state_of(db, vid, uid).await {
        Ok(s) => unusable_reason(&s)
            .unwrap_or_else(|| "券状态刚发生变化，请刷新后重试".to_string()),
        Err(e) => {
            tracing::warn!(err = %e, voucher = vid, "读取券状态失败");
            "券不存在、已使用或已过期".to_string()
        }
    }
}

#[cfg(test)]
mod voucher_state_tests {
    use super::{VoucherState, unusable_reason};

    #[test]
    fn available_has_no_error() {
        assert_eq!(unusable_reason(&VoucherState::Available), None);
    }

    /// 这条是本批的本体：绑定生效中的券，三句旧指控一句都不许出现
    #[test]
    fn bound_is_not_called_used_or_expired() {
        let msg = unusable_reason(&VoucherState::Bound(40)).unwrap_or_default();
        assert!(!msg.contains("已使用"), "actual: {msg}");
        assert!(!msg.contains("已过期"), "actual: {msg}");
        assert!(!msg.contains("不存在"), "actual: {msg}");
        assert!(msg.contains("#40"), "actual: {msg}");
    }

    #[test]
    fn other_states_name_their_own_reason() {
        assert_eq!(unusable_reason(&VoucherState::Used).as_deref(),
                   Some("该券已核销"));
        assert_eq!(unusable_reason(&VoucherState::Expired).as_deref(),
                   Some("该券已过有效期"));
        let m = unusable_reason(&VoucherState::Missing).unwrap_or_default();
        assert!(m.contains("不存在"), "actual: {m}");
        assert!(m.contains("不属于当前账号"), "actual: {m}");
    }
}

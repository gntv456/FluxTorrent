//! 周配额原子占位（从 invite_http.rs issue 口拆出，行数门禁）。
//! INSERT 计数行（并发首到者建）+ 条件 UPDATE 占位（used < quota 才 +1）。

use actix_web::web;

use crate::authz::perm::INVITES_BONUS as PERM_BONUS;
use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::state::AppState;

/// 周配额的周期口径（北京时间自然周，0204）：多处 SQL 共用，收口一处。
pub(super) const WEEK_PERIOD: &str =
    "date_trunc('week', now() AT TIME ZONE 'Asia/Shanghai')::date";

/// 判定用户邀请配额上限（与消耗路径同一口径）：INVITES_BONUS 权限
/// （外联员/VIP）与 LV3+ 的每周配额——0204 起读 site_settings
/// （invites_weekly_bonus / invites_weekly_class3）。
pub(super) async fn quota_limit(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
) -> i64 {
    let key = if crate::authz::can(state, auth, PERM_BONUS).await {
        "invites_weekly_bonus"
    } else if auth.class_id >= 3 {
        "invites_weekly_class3"
    } else {
        return 0;
    };
    super::setting_i64(&state.repo.db, key, 2)
        .await
        .clamp(0, 100)
}

/// 额外配额耗尽后的周配额占位：拿不到即 Err（等级不足/本周用完）。
pub(super) async fn take_week_quota(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
) -> DomainResult<()> {
    let quota = quota_limit(state, auth).await;
    if quota == 0 {
        return Err(DomainError::Validation(
            "等级达到 LV3 后才能生成邀请码；也可用魔力兑换（不受等级限制）"
                .into(),
        ));
    }
    sqlx::query(&format!(
        "INSERT INTO invite_quota (user_id, period, used) \
         VALUES ($1, {}, 0) ON CONFLICT DO NOTHING",
        WEEK_PERIOD
    ))
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let taken: Option<i32> = sqlx::query_scalar(&format!(
        "UPDATE invite_quota SET used = used + 1 \
         WHERE user_id = $1 AND period = {} AND used < $2 \
         RETURNING used",
        WEEK_PERIOD
    ))
    .bind(auth.id)
    .bind(quota)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if taken.is_none() {
        return Err(DomainError::Validation(
            "本周邀请配额已用完，下周重置；也可用魔力兑换".into(),
        ));
    }
    Ok(())
}

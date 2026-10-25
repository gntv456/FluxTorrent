//! 申诉受理后的撤销警告（2026-10-25 版主九轮，与 `appeal_unban` 同一口径）。
//!
//! 缺陷本体：`kind='warn'` 的申诉受理只翻 `appeals.status` 并发一句
//! 「你的申诉已受理」，而 `users.warned_until/warned_reason` 原样留着。
//! 读这一列的地方都因此继续把当事人当「背着警告」的人对待：
//!   * 后台「警告用户」面板 `GET /admin/warned`——`staff_http/warned.rs:24`
//!     只列 `warned_until > now()`，九轮实测：受理后该行仍在列表里；
//!   * 管理端用户详情的警告条 `admin-user-detail-panels.tsx:77`；
//!   * 处置公示名单 `GET /ban-log`（`ban-log-list.tsx:83` 取
//!     `cheat_banned_until ?? warned_until` 当「期限至」）——其谓词是
//!     `u.status IN (1,2)`（`repo/community/ban_log.rs:56`），故只有当事人
//!     同时被禁言/封禁时才露面，届时显示的期限就是这条警告的到期日。
//! 于是职员按了「受理」还得再去点一次删除；少点一次，就是
//! **回复说已受理、警告照挂**。
//!
//! 三条护栏与封禁联动同源，不另立判据：
//!   ① 操作者等级需严格高于目标——挂警告有 `ensure_outranks`
//!      （0285 实测补），三轮审计 P1-2 又给解警告补齐同一护栏；
//!   ② 只清真背着警告的行（`warned_until IS NOT NULL`）——没人背警告时
//!      返回 false，调用方据此在回执里说实话，不谎称「已撤销」；
//!   ③ 动作进审计（`appeal.unwarn`），与 `warn_user`/`unwarn_user` 并列可查。

use actix_web::web;
use std::sync::Arc;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 受理「警告申诉」后的撤销：先验等级护栏 → 清 warned_until/warned_reason → 写审计。
/// 返回是否真撤销了一条在册警告。
pub(crate) async fn lift_if_outranked(
    state: &web::Data<Arc<AppState>>,
    actor_id: i64,
    actor_class: i32,
    user_id: i64,
) -> DomainResult<bool> {
    crate::admin_http::guard::ensure_outranks(
        &state.repo.db,
        actor_class,
        user_id,
    )
    .await?;
    let lifted = sqlx::query(
        "UPDATE users SET warned_until = NULL, warned_reason = NULL \
         WHERE id = $1 AND warned_until IS NOT NULL",
    )
    .bind(user_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if lifted > 0 {
        state
            .repo
            .audit(Some(actor_id), "appeal.unwarn", Some(user_id))
            .await;
    }
    Ok(lifted > 0)
}

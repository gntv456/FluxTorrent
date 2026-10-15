//! H&R 赦免用例（FRAME-PLAN-01 F2 示范抽离之二，批四十三 P-165~P-167）。
//!
//! 单发口（`gaps_http::hr`）、gaps 批量口、`admin_p3_http::hr` 批量口三处
//! 共用同一套判据与副作用；此前这份判据住在 **H 层**（`admin_p3_http/hr.rs`
//! 的 `pardon_one`），签名里带 `web::Data` 与 `&AuthUser`——用例层不该知道
//! HTTP 请求上下文。本文件按 F2 示范（`services/review_decide.rs`）的做法
//! 抽到服务层：签名只收纯数据（操作者 id / class / 目标 / 理由），路由层
//! 负责鉴权与形状映射。
//!
//! 判据三条（与抽离前逐字一致，SQL 自 H 层原样搬移，行为零变）：
//!   ① **操作者须严格高于目标**。成员侧同一动作 `/me/hr/pardon` 要付站点设定
//!      `cancel_hr` 的火花，而管理口谁都能按 ⇒ 有 `hr.pardon` 权的职员
//!      可零成本赦免自己那条违规。`ensure_outranks` 里 `actor <= target`
//!      即拒，顺带把「赦自己」一起挡掉。
//!   ② **只赦 `violated`**。`open` 是「还没到期」的进行中义务，worker 的
//!      判定线是 `status='open' AND deadline < now()`；抹成 pardoned 后这条线
//!      永远不再命中，义务凭空消失，而 `hr_violations` 里连依据都没有。
//!   ③ **理由必须落库**。进审计 `audit_detail`，并给被赦的人留一条
//!      `user_modify_logs`（纠纷仲裁时看得见）。
//!
//! 返回 `true` = 真赦免了一条；`false` = 没有待赦记录（调用方决定报 400
//! 还是计入 skipped，响应形状不变）。`action` 由调用方给（单发/批量在台账里
//! 仍是两个动作名，`apps/web/i18n/audit-labels.ts` 已各自有标签）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 赦免一条 H&R 违规的全部判据与副作用。
///
/// 参数刻意是纯数据而非 `AuthUser`：服务层不认 HTTP 上下文
/// （`layer_guard` 规则 ② 对本目录零容忍）。
pub(crate) async fn pardon_one(
    state: &AppState,
    actor_id: i64,
    actor_class: i32,
    action: &str,
    uid: i64,
    tid: i64,
    note: &str,
) -> DomainResult<bool> {
    crate::admin_http::guard::ensure_outranks(
        &state.repo.db,
        actor_class,
        uid,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, \
         updated_at = now() WHERE user_id = $2 AND torrent_id = $3 \
         AND status = 'violated'",
    )
    .bind(actor_id)
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Ok(false);
    }
    // 展示层读 snatches.hr_flag，而 worker 只置 TRUE（全库无 FALSE 路径）
    // ⇒ 赦免后不回清，用户侧 H&R 角标会一直亮着
    sqlx::query(
        "UPDATE snatches SET hr_flag = FALSE WHERE user_id = $1 \
         AND torrent_id = $2",
    )
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
    )
    .bind(actor_id)
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(actor_id),
            action,
            Some(tid),
            Some(note),
            Some(serde_json::json!({ "target_user": uid })),
        )
        .await;
    crate::admin_p3_http::meta::modify_log(
        &state.repo.db,
        uid,
        Some(actor_id),
        &format!("H&R 违规已赦免（种 {tid}）：{note}"),
    )
    .await;
    Ok(true)
}

/// 批量前的整批护栏预扫：任一目标不可赦免就**一个都不动**地拒掉，
/// 免得留下「赦了一半、另一半被权限挡下」的中间态（`pardon_one` 内部
/// 还会再判一次，同一份判据）。
pub(crate) async fn ensure_all_pardonable(
    state: &AppState,
    actor_class: i32,
    ids: &[i64],
) -> DomainResult<()> {
    for uid in ids {
        crate::admin_http::guard::ensure_outranks(
            &state.repo.db,
            actor_class,
            *uid,
        )
        .await?;
    }
    Ok(())
}

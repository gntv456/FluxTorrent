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
    crate::admin_http::guard::ensure_outranks(&state.repo.db, actor_class, uid)
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

/// H&R 免罪费给发布者的分成（0424，同类站对标 A1）。
///
/// 钱只从**已扣的免罪费**里分——不是新的铸币口，分成上限即免罪费本身。
/// 与 同类站 同口径的三个不分成例外：
///   * 比例键为 0（站长关了分成）；
///   * 发布者就是受罚者本人（自发种免罪）；
///   * 发布者账号异常（status >= 2 即被禁/删，钱进系统回收）。
/// 幂等键锚定免罪扣费键（`hr-self-pardon:{uid}:{tid}:share`）——免罪重放
/// 在上游 spend 处已被拒，这里的键天然不会撞。
pub(crate) async fn share_to_publisher(
    db: &sqlx::PgPool,
    pardoned_user: i64,
    torrent_id: i64,
    fee: i64,
    fee_idem: &str,
) -> DomainResult<i64> {
    if fee <= 0 {
        return Ok(0);
    }
    let pct: i64 = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'hr_pardon_share_pct'), '')::bigint, 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30)
    .clamp(0, 100);
    if pct == 0 {
        return Ok(0);
    }
    let share = fee * pct / 100;
    if share == 0 {
        return Ok(0);
    }
    // 发布者须在册且未被封禁；自发种不分成（owner = 受罚者）。
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT t.owner_id FROM torrents t JOIN users u ON u.id = t.owner_id \
         WHERE t.id = $1 AND u.status < 2 AND t.owner_id <> $2",
    )
    .bind(torrent_id)
    .bind(pardoned_user)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Ok(0);
    };
    crate::economy_http::earn_spark(
        db,
        owner,
        share,
        "hr_pardon_share",
        &format!("{fee_idem}:share"),
    )
    .await?;
    Ok(share)
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

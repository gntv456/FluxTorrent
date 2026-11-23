//! 详情聚合的「保种认领位」（从 aggregate.rs 拆出，2026-11-23）。
//!
//! 详情页头部的认领按钮需要三态：种子是否在保种区、本人是否已认领、
//! 是否已被任何人认领。这是 viewer 态（claimed_by_me 因人而异），所以
//! 由 `aggregate::assemble` 的个人段实时取，不进 30s 共享缓存。
//!
//! 尺子与保种区列表 / `POST /preserve/claim` 完全同一把
//! （`approval_status = 1` 且 `exited_at IS NULL`，P-317 口径）——
//! 列表里看不见的种子，详情页也不给认领按钮，不会出现「列表没有、
//! 详情却能领」的两把尺打架。

use serde::Serialize;

/// 保种认领三态。`seed_preserve` 无行 / 已移出 / 未过审 → 调用方拿到
/// `None`，详情页不渲染按钮。
#[derive(Serialize, sqlx::FromRow)]
pub(super) struct PreserveClaimState {
    in_zone: bool,
    claimed_by_me: bool,
    claimed: bool,
    /// 每保满一天可得（preserve_bonus_per_day，认领按钮利益角签用；
    /// 与保种区列表 `preserve.rs` 同一配置同一缺省）
    bonus_per_day: i64,
}

/// 取某用户视角下某颗种子的保种认领态。
///
/// 读失败按「不在区」降级（返回 `None`）：详情页只是少一个按钮，
/// 不该让整页报错。
pub(super) async fn preserve_state(
    db: &sqlx::PgPool,
    torrent_id: i64,
    uid: i64,
) -> Option<PreserveClaimState> {
    // 三态表达式必须 COALESCE 成非 NULL：未认领时 claimed_by IS NULL，
    // `claimed_by = $2` 的结果是 NULL 而不是 false；sqlx FromRow 拿 NULL
    // 填 bool 会报错 → 整行被吞成 None，按钮永远不渲染（实测踩坑）。
    let row: Option<PreserveClaimState> = sqlx::query_as(
        "SELECT COALESCE(t.approval_status = 1 \
                      AND sp.exited_at IS NULL, false) AS in_zone, \
                COALESCE(sp.claimed_by = $2, false) AS claimed_by_me, \
                COALESCE(sp.claimed_by IS NOT NULL, false) AS claimed, \
                COALESCE((SELECT value::bigint FROM site_settings \
                          WHERE name = 'preserve_bonus_per_day'), 100) \
                    AS bonus_per_day \
         FROM torrents t \
         LEFT JOIN seed_preserve sp ON sp.torrent_id = t.id \
         WHERE t.id = $1 AND sp.torrent_id IS NOT NULL",
    )
    .bind(torrent_id)
    .bind(uid)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    row.filter(|r| r.in_zone)
}

//! 详情聚合的个人段数据访问（`torrent_http/aggregate` 的 SQL 唯一居所）。
//! 2026-11-24 下沉：FRAME §2「H 不 import sqlx」——分层门禁对新 H 层文件
//! 带 SQL 零容忍，拆件时不能把 SQL 从基线内的旧文件搬进新 H 层文件。
//!
//! 共享段（torrent/detail/files/nfo/tags）按 (id, reveal) 进 30s Redis 缓存、
//! 跨用户复用；本模块只管**因人而异**、不能进共享缓存的两种取数：
//!   ① 当前用户对这颗种子的四个 viewer 态布尔
//!   ② 保种认领位（尺子与保种区列表 / `POST /preserve/claim` 同一把，
//!      P-317 口径）
//! 逐字搬运自原 `torrent_http/aggregate.rs::assemble`，语义不变。

use serde::Serialize;

use crate::repo::Repo;

/// 当前用户对这颗种子的 viewer 态：`(is_owner, purchased, bookmarked, thanked)`。
///
/// 共享缓存里存的是「第一个访问者」的答案，必须按当前用户重算
/// （0285 修过一次，被并行批次覆盖回 `let _ = uid;`，症状：B 打开付费种
/// 看到「已购买/我发布」，付费墙被隐藏）。重算失败宁可回落保守值
/// （非本人/未购买），也不能端出缓存里别人的答案——那正是 0285 的症状
/// 会在 DB 抖动时复现的路径。
pub async fn viewer_flags(
    repo: &Repo,
    id: i64,
    uid: i64,
) -> (bool, bool, bool, bool) {
    sqlx::query_as::<_, (bool, bool, bool, bool)>(
        "SELECT (t.owner_id = $2) AS is_owner, \
                EXISTS(SELECT 1 FROM torrent_purchases p \
                        WHERE p.torrent_id = t.id \
                          AND p.user_id = $2) AS purchased, \
                EXISTS(SELECT 1 FROM bookmarks b \
                        WHERE b.torrent_id = t.id \
                          AND b.user_id = $2) AS bookmarked, \
                EXISTS(SELECT 1 FROM thanks th \
                        WHERE th.torrent_id = t.id \
                          AND th.user_id = $2) AS thanked \
         FROM torrents t WHERE t.id = $1",
    )
    .bind(id)
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((false, false, false, false))
}

/// 保种认领三态。`seed_preserve` 无行 / 已移出 / 未过审 → `None`，
/// 详情页不渲染认领按钮。
#[derive(Serialize, sqlx::FromRow)]
pub struct PreserveClaimState {
    in_zone: bool,
    claimed_by_me: bool,
    claimed: bool,
    /// 每保满一天可得（preserve_bonus_per_day，认领按钮利益角签用；
    /// 与保种区列表 `ops_http/preserve.rs` 同一配置同一缺省）
    bonus_per_day: i64,
}

/// 取某用户视角下某颗种子的保种认领态。
///
/// 读失败按「不在区」降级（返回 `None`）：详情页只是少一个按钮，
/// 不该让整页报错。
pub async fn preserve_claim_state(
    repo: &Repo,
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
    .fetch_optional(&repo.db)
    .await
    .ok()
    .flatten();
    row.filter(|r| r.in_zone)
}

//! 促销 + 券倍率裁决（2026-10-07 从 process_event 抽出，300 行门禁）。

use super::announce_main::AnnounceEvent;
use super::billing::billing_multipliers;
use sqlx::PgPool;

/// 促销 + 券倍率裁决（计费前置段抽出的纯读路径）：
/// 种子专属促销 → 全局促销（official/non_official/category 维度）→ free/neutral
/// 券叠加。全部按**事件时点**裁决，积压重放不按过期价补计费。
/// $1 三占位符三 bind / $1$4 显式引用的注释见原内联版（P0 修复存档）。
pub(crate) async fn resolve_billing_mults(
    db: &PgPool,
    ev: &AnnounceEvent,
    torrent_id: i64,
) -> anyhow::Result<(f64, f64, Option<String>, Option<String>)> {
    // 促销快照裁决（§5.4-⑦）——与 API 展示口径一致：同种子多条专属促销取最强档
    // （修复前 ORDER BY id DESC 只认最新一条：先挂 free 后挂 half 时计费取 half、展示取 free）
    // 裁决时点 = 事件时点 ev.ts（缺省 now）：积压重放时不再按过期后的价目补计费
    let ev_time = ev.ts.unwrap_or_else(chrono::Utc::now);
    // 审计修复（P0）：专属促销查询旧版把 $1 重复引用三次并 bind 三次 —— PG 扩展协议按
    // 「最大占位符编号」要求 4 个参数，但未在 SQL 中出现的编号无法推断类型，
    // Parse 阶段报 "could not determine data type of parameter $2"，每个 announce
    // 事件重试 6 次进 DLQ，计费链路整体瘫痪。改为 $1/$4 显式引用 + 仅 bind 两个参数。
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions \
         WHERE torrent_id = $1 AND starts_at <= $2 \
           AND ends_at > $2 \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 \
             WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
             WHEN 'free' THEN 3 WHEN 'half' THEN 2 \
             WHEN 'p30' THEN 1 ELSE 0 END DESC, id DESC \
         LIMIT 1",
    )
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    // 审计修复（P0 真根因，PG 日志实锄）：旧 SQL 里 $1 出现 3 次（三个 EXISTS 子查询），
    // Rust 侧只 bind 1 个参数 —— sqlx Describe/Bind 参数计数协商失败后发出 0 参数 Bind，
    // "supplies 0 parameters" 每轮必炸，announce 计费自 07-11 起整体瘫痪。改写为 $1 单次引用。
    let global: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions p \
         WHERE p.torrent_id IS NULL AND p.starts_at <= $4 AND p.ends_at > $4 \
           AND (p.scope = 'global' \
                OR (p.scope = 'official' AND (SELECT official_tag \
                     FROM torrents WHERE id = $1)) \
                OR (p.scope = 'non_official' AND NOT (SELECT official_tag \
                     FROM torrents WHERE id = $2)) \
                OR (p.scope = 'category' AND p.category_id = \
                     (SELECT category_id FROM torrents WHERE id = $3))) \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 \
             WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
             WHEN 'free' THEN 3 WHEN 'half' THEN 2 \
             WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC \
         LIMIT 1",
    )
    // 同值三占位符 + 三 bind（sqlx 按占位符种类计数；缺 bind 会 0 参数发送）
    .bind(torrent_id)
    .bind(torrent_id)
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) =
        billing_multipliers(kind.as_deref(), global.as_deref());

    // 0073 券倍率叠加：free 券 → 下载计 0；neutral 券 → 上下行均计 0。
    // 判定口径：本人该种存在绑定中（used_at 仍 NULL）的对应 kind 券；过期判定同促销用事件时点。
    // 与促销取更优（乘法叠加：促销 x2 上传对 neutral 也归零，取对用户更优的 0）。
    let voucher: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM user_vouchers \
         WHERE user_id = $1::bigint AND used_torrent_id = $2::bigint \
           AND used_at IS NULL AND expires_at > $3 \
         ORDER BY CASE kind WHEN 'neutral' THEN 2 WHEN 'free' THEN 1 \
         ELSE 0 END DESC LIMIT 1",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) = match voucher.as_deref() {
        Some("neutral") => (0.0, 0.0),
        Some("free") => (up_mult, 0.0),
        _ => (up_mult, down_mult),
    };
    Ok((up_mult, down_mult, kind, global))
}

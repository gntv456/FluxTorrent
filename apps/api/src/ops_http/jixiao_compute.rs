//! 绩效指标计算引擎（M21）：compute_metrics + jixiao_bonus。
//! 从 ops_http/jixiao.rs 按域拆出。

use crate::errors::{DomainError, DomainResult};

use super::jixiao::jixiao_prev_period;

/// 指标采集：全部来自系统流水（announce 统计/保种表/操作日志），零手工填报（M21 验收）。
///
/// 0106 口径：
///   做种时长类用「月度基线快照差值」（jixiao_baseline_snapshots，主口径；
///   快照缺失时用 jixiao_claims 行级 base_* 兜底——admin 分配时记的当刻累计值）
///   现值类（seeding_count/seed_size/seed_size_tb）不参与差值
///   ops 修复为按月过滤（旧实现是全历史计数，口径不符）
pub async fn compute_metrics(
    db: &sqlx::PgPool,
    user_id: i64,
    period: &str, // "2026-09"
) -> DomainResult<serde_json::Value> {
    // ── 期初基线：月度快照优先，缺则行级 base_*（admin 分配时记），再缺则 0 ──
    // 只需做种时长基线（seed_hours 差值）；流量/发布数直接按 traffic_ledger/torrents
    // 的当月聚合，不依赖基线。
    let prev = jixiao_prev_period(period);
    let mut base_seed: Option<i64> = None;
    if let Some(p) = &prev {
        base_seed = sqlx::query_scalar(
            "SELECT seed_seconds FROM jixiao_baseline_snapshots \
             WHERE user_id = $1 AND period = $2",
        )
        .bind(user_id)
        .bind(p)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    }
    let base_seed: i64 = match base_seed {
        Some(v) => v,
        None => sqlx::query_scalar(
            "SELECT base_seed_seconds FROM jixiao_claims \
             WHERE user_id = $1 AND period = $2 AND metrics_snapshot->>'source' = 'admin' \
             ORDER BY id LIMIT 1",
        )
        .bind(user_id)
        .bind(period)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten()
        .unwrap_or(0),
    };

    // ── 当月流量（traffic_ledger 按小时窗，月度聚合；基线差值口径对齐） ──
    let (up_month, down_month): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint \
         FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or((0, 0));

    // ── 做种时长：现值 - 期初基线（snatches.seeded_seconds 是 int，显式 ::bigint） ──
    let seed_seconds_now: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(seeded_seconds), 0)::bigint FROM snatches \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_hours = ((seed_seconds_now - base_seed).max(0)) / 3600;

    // ── 当月做种活动天数（近似）：snatches.last_seen_at 按天去重 ──
    let seed_days: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT date_trunc('day', s.last_seen_at)) FROM snatches s \
         WHERE s.user_id = $1 AND to_char(s.last_seen_at, 'YYYY-MM') = $2 AND s.seeding",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let seeding_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id \
         = $1 AND seeding",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size),0)::bigint FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 AND s.seeding",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let uploads: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 AND \
         to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    // ops 修复：按月过滤（旧实现全历史计数，与「当月操作数」考核口径不符）
    let ops: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND \
         to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // 平均做种时长（NP Exam 口径）：时长差值 ÷ 期内有做种活动的种子数（无则 1）
    let active_seed_torrents: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT torrent_id) FROM snatches \
         WHERE user_id = $1 AND to_char(last_seen_at, 'YYYY-MM') = $2 AND seeding",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let avg_seed_hours = seed_hours / active_seed_torrents.max(1);

    // 火花增量（当月正向流水合计）与做种积分增量（1 积分 = 1 小时，与 exam 引擎同源）
    let spark_delta: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount),0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND amount > 0 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "uploaded": up_month, "downloaded": down_month, "uploads": uploads,
        "seeding_count": seeding_count, "seed_size": seed_size,
        "seed_size_tb": seed_size / 1_099_511_627_776, // 1 TB = 2^40
        "seed_hours": seed_hours, "seed_days": seed_days, "ops": ops,
        "avg_seed_hours": avg_seed_hours,
        "seed_points_delta": seed_hours, // 1 积分 = 1 小时做种（task_jobs.rs P1-6 口径）
        "spark_delta": spark_delta,
    }))
}

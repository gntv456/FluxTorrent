//! 绩效结算与快照滚动。
//! 从 jobs.rs 按域拆出。

use super::jixiao_snap::jixiao_rollover_snapshot;
use sqlx::PgPool;

/// 绩效考核月末结算（0106，参考各 PT 站工作组考核口径）。
///
/// 每天 tick 一次，幂等护栏：
///   * 只处理**上一期**（站点时区 UTC+8 已跨月）`source='admin'` 且 `settled_at IS NULL` 的登记行
///   * 「settled_at IS NULL 作未处理标记 + 状态推进与发奖同一事务」（social_team_settle 同款，
///     崩溃整体回滚下轮重来；比旧 resurrection_settle 的「先 CAS 置 done 后发奖」可靠）
///   * 发薪幂等键 `jixiao:settle:{claim_id}`，先查后插防重复
///
/// 结算语义：
///   * 达标（min_requirements 全部满足）→ status=1 + 发 base_pay+加成（达标月数含本期）
///   * 未达标 → status=2 + 平实文案 PM（不羞辱：已产生的做种收益不受影响）
///   * 同轮为下一期落全站活跃用户基线快照（jixiao_baseline_snapshots，
///     PK(user_id,period) 幂等）——compute_metrics 的 seed_hours 等差值口径依赖它
///   * 本 job 不检查任何模块开关（已分配的岗位必须能收尾，social 层同款约定）
///
/// 注意：用户在补领窗口（jixiao_claim_window_days，默认 7 天）内仍可自领当期
/// ——worker 结算先到先得，两边都以「settled_at IS NULL + 单事务 CAS」互斥。
pub async fn jixiao_settle(db: &PgPool) -> anyhow::Result<u64> {
    // 站点时区（UTC+8）当前月份；只有跨月后上一期才可结算
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let cur: String = now_site.format("%Y-%m").to_string();
    let prev: String = {
        let (y, m): (i32, u32) = {
            let mut it = cur.split('-');
            let y = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let m = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            (y, m)
        };
        if y == 0 || !(1..=12).contains(&m) {
            return Ok(0);
        }
        if m == 1 {
            format!("{:04}-12", y - 1)
        } else {
            format!("{y:04}-{:02}", m - 1)
        }
    };

    // 上一期未结算的 admin 登记行（含用户/岗位信息）
    let pending: Vec<super::jixiao_loop::PendingClaim> = sqlx::query_as(
        "SELECT c.id, c.user_id, c.type_id, t.name, t.base_pay, t.min_requirements, t.bonus_rules, \
                c.base_seed_seconds, c.base_uploaded, c.base_uploads \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.period = $1 AND c.metrics_snapshot->>'source' = 'admin' \
           AND c.settled_at IS NULL AND c.status = 0",
    )
    .bind(&prev)
    .fetch_all(db)
    .await?;
    if pending.is_empty() {
        // 没有登记行也要保证基线快照滚动（新月份第一次 tick 落当期快照）
        return jixiao_rollover_snapshot(db, &cur).await.map(|_| 0);
    }

    // 加成参数：全站 site_settings 兜底（岗位级 bonus_rules 优先，逐岗位在循环内取）
    let site_step: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name='jixiao_bonus_months_per_step'), 3)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(3)
    .max(1);
    let site_pct: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name='jixiao_bonus_percent_per_step'), 10)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(10);

    let done = super::jixiao_loop::jixiao_settle_loop(
        db, pending, &prev, site_step, site_pct,
    )
    .await?;
    // 下一期（=当前期）基线快照：结算完毕后滚动
    jixiao_rollover_snapshot(db, &cur).await?;
    if done > 0 {
        tracing::info!(done, period = %prev, "jixiao settled");
    }
    Ok(done)
}

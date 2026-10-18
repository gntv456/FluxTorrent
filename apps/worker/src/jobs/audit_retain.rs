//! 审计日志保留策略（0357）：超期 audit_log 行归档到 audit_log_archive。
//!
//! 与 sweep.rs 硬删 login_events 不同：audit_log 是防篡改哈希链（0266），
//! DELETE 会被触发器拦死——必须照 0266 存量回填的三段式：事务内
//! DROP TRIGGER → DELETE → 重建触发器，并以 audit_chain_anchors 记断点，
//! chain-verify 据此对存活首行改对照锚（apps/api/src/admin_http/audit.rs）。

use redis::AsyncCommands;
use sqlx::PgPool;

/// 单次归档上限：控事务时长；超出部分留下一轮（每天一批，见节流键）。
pub(crate) const AUDIT_ARCHIVE_BATCH: i64 = 500_000;

/// Redis 日节流键：实际动作每天最多一次（job 本身每小时挂 hour_tick）。
const THROTTLE_KEY: &str = "flux:audit_retain:done";

/// 读 audit_retain_days（0/缺省非法值=永久关闭归档；负数同 0 处理）。
pub(crate) async fn audit_retain_days(db: &PgPool) -> i64 {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'audit_retain_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(0)
    .max(0)
}

/// cutoff 选择：超期窗口内最大 id；返回 None = 无行可归档。
/// （测试可见的最小决策面：0 天/无旧行两分支都在此返回 None。）
pub(crate) fn pick_cutoff(
    retain_days: i64,
    max_expired_id: Option<i64>,
) -> Option<i64> {
    if retain_days <= 0 {
        return None; // 永久保留
    }
    max_expired_id
}

/// 分批上限：候选 cutoff 若会把本批推过上限，压到第 BATCH 万行的 id。
pub(crate) fn clamp_cutoff(cutoff: i64, batch_nth_id: Option<i64>) -> i64 {
    match batch_nth_id {
        Some(nth) if nth < cutoff => nth,
        _ => cutoff,
    }
}

/// run.rs hour_tick 挂载点（shard_lock!("job:audit_retain") 包装）：
/// Redis 日节流 + 一批归档都在 audit_retain 内。
pub(crate) async fn audit_retain_tick(
    db: &PgPool,
    redis: &redis::aio::ConnectionManager,
) -> anyhow::Result<()> {
    let mut r = redis.clone();
    audit_retain(db, &mut r).await.map(|_| ())
}

/// 归档主入口：hour_tick 调度（shard_lock!("job:audit_retain") 已在
/// run.rs 包装），体内先过 Redis SET NX EX 86400 日节流。
pub(crate) async fn audit_retain(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    let retain = audit_retain_days(db).await;
    if pick_cutoff(retain, Some(0)).is_none() {
        return Ok(0); // 0=永久关闭归档（口径同 0230 ledger_retain）
    }
    // 日节流：抢不到键=今天已归档过。归档失败不占坑（先 NX 后做事，
    // 失败时删键让下一小时重试——与 dlq 告警键同模式但可回收）。
    // redis 0.27 无 set_nx_ex 便捷方法，走 SetOptions（NX + EX 86400）。
    let nx: Option<i64> = redis
        .set_options(
            THROTTLE_KEY,
            1i64,
            redis::SetOptions::default()
                .conditional_set(redis::ExistenceCheck::NX)
                .with_expiration(redis::SetExpiry::EX(86_400)),
        )
        .await
        .ok()
        .flatten();
    if nx.is_none() {
        return Ok(0); // 键已存在（NX 落空返回 nil）= 今天已归档
    }
    match archive_once(db, retain).await {
        Ok(n) => Ok(n),
        Err(e) => {
            let _: Result<(), _> = redis.del(THROTTLE_KEY).await;
            Err(e)
        }
    }
}

/// 一批归档（单事务）：INSERT 归档 → 三段式 DELETE → 记锚点。
async fn archive_once(db: &PgPool, retain: i64) -> anyhow::Result<u64> {
    // 窗口内最大 id（严格小于保留边界：边界当天的行还差 N 小时才满期，
    // 与 login_events 的 now()-interval 同口径）。
    let max_expired: Option<i64> = sqlx::query_scalar(
        "SELECT max(id) FROM audit_log \
         WHERE created_at < now() - ($1::bigint * interval '1 day')",
    )
    .bind(retain)
    .fetch_one(db)
    .await?;
    let Some(cutoff_raw) = pick_cutoff(retain, max_expired) else {
        return Ok(0); // 无超期行
    };
    // 分批：候选行数超上限时压到第 BATCH 行的 id（下轮继续）。
    let batch_nth: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM audit_log WHERE id <= $1 \
         ORDER BY id OFFSET $2 LIMIT 1",
    )
    .bind(cutoff_raw)
    .bind(AUDIT_ARCHIVE_BATCH - 1)
    .fetch_optional(db)
    .await?;
    let cutoff = clamp_cutoff(cutoff_raw, batch_nth);

    let mut tx = db.begin().await?;
    // 删除前取 cutoff 行 self_hash（锚点对照物；行不存在=窗口空，早退）
    let cutoff_hash: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT self_hash FROM audit_log WHERE id = $1")
            .bind(cutoff)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(cutoff_self_hash) = cutoff_hash else {
        tx.rollback().await?;
        return Ok(0);
    };
    let archived = sqlx::query(
        "INSERT INTO audit_log_archive SELECT * FROM audit_log \
         WHERE id <= $1 ON CONFLICT (id) DO NOTHING",
    )
    .bind(cutoff)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    // 三段式（0266 先例）：DROP 防篡改触发器 → DELETE → 重建。
    sqlx::query("DROP TRIGGER IF EXISTS trg_audit_log_chain ON audit_log")
        .execute(&mut *tx)
        .await?;
    let deleted = sqlx::query("DELETE FROM audit_log WHERE id <= $1")
        .bind(cutoff)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    sqlx::query(
        "CREATE TRIGGER trg_audit_log_chain \
         BEFORE INSERT OR UPDATE OR DELETE ON audit_log \
         FOR EACH ROW EXECUTE FUNCTION audit_log_chain()",
    )
    .execute(&mut *tx)
    .await?;
    // 锚点（0357）：chain-verify 对新首行 prev_hash 对照此 hash。
    sqlx::query(
        "INSERT INTO audit_chain_anchors \
         (cutoff_id, cutoff_self_hash, archived_rows) VALUES ($1, $2, $3)",
    )
    .bind(cutoff)
    .bind(&cutoff_self_hash)
    .bind(archived as i64)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if archived > 0 {
        tracing::info!(
            cutoff,
            archived,
            deleted,
            "审计日志归档完成（锚点已记）"
        );
    }
    Ok(archived)
}

#[cfg(test)]
mod tests {
    use super::{clamp_cutoff, pick_cutoff};

    /// 0 天=永久关闭归档：即便窗口内有超期行也必须跳过
    #[test]
    fn cutoff_skips_when_retain_zero() {
        assert_eq!(pick_cutoff(0, Some(12345)), None);
        assert_eq!(pick_cutoff(-1, Some(12345)), None);
    }

    /// 保留期>0 但窗口无行（max(id) 为 NULL）→ 跳过
    #[test]
    fn cutoff_skips_when_no_expired_rows() {
        assert_eq!(pick_cutoff(180, None), None);
    }

    /// 正常路径：窗口内最大 id 即 cutoff
    #[test]
    fn cutoff_takes_max_expired_id() {
        assert_eq!(pick_cutoff(180, Some(777)), Some(777));
    }

    /// 分批上限：第 50 万行 id 更小则压缩 cutoff（余量下轮继续）
    #[test]
    fn batch_clamps_to_nth_id() {
        assert_eq!(clamp_cutoff(900_000, Some(500_000)), 500_000);
        // 候选不足一批（第 50 万行不存在或等于 cutoff）→ 不压
        assert_eq!(clamp_cutoff(300_000, Some(300_000)), 300_000);
        assert_eq!(clamp_cutoff(300_000, None), 300_000);
    }
}

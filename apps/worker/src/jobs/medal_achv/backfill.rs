//! 回填：给存量用户扫一轮，分批可续跑。

use sqlx::PgPool;

use super::eval::{event_total, eval_threshold_one, prog};
use super::{award, pending_users, save_progress, RuleRow, BATCH, MAX_BATCHES};
use super::api_domain as md;

/// 回填单个人（event / threshold 两条路径）。返回是否新授予。
async fn backfill_one(
    db: &PgPool,
    r: &RuleRow,
    user_id: i64,
) -> anyhow::Result<bool> {
    let mode = md::Mode::parse(r.auto_mode.as_deref());
    match mode {
        md::Mode::Threshold => eval_threshold_one(db, r, user_id).await,
        md::Mode::Event => {
            // event 模式回填：从 medal_events 聚合已发生的量。
            let total =
                event_total(db, user_id, r.auto_rule.as_deref().unwrap_or(""))
                    .await;
            let cur = prog(db, user_id, r.id).await;
            let rule = md::Rule {
                mode,
                period: md::Period::Total,
                target: r.auto_target.unwrap_or(0),
            };
            match md::judge_event(&rule, cur, total - cur) {
                md::Verdict::Award => award(db, user_id, r, "event").await,
                _ => {
                    save_progress(db, user_id, r.id, total).await?;
                    Ok(false)
                }
            }
        }
        md::Mode::Manual => Ok(false),
    }
}

/// 回填推进。返回 `(已标记完成的成就数, 授予人数)`。
///
/// 分批 + 「已授予即跳过」两条共同保证可中断可续跑：中途被 kill
/// 不会丢掉「扫到哪了」——下轮从还没拿到的第一个人接着扫。
pub(crate) async fn backfill(db: &PgPool) -> anyhow::Result<(u64, u64)> {
    let rules: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, name, auto_rule, auto_mode, auto_target, auto_period \
         FROM medals WHERE active = TRUE AND auto_mode IS NOT NULL \
         AND NOT backfill_done ORDER BY id",
    )
    .fetch_all(db)
    .await?;
    let mut done_rules = 0u64;
    let mut granted = 0u64;
    for r in &rules {
        let mut batches = 0i32;
        loop {
            let batch = pending_users(db, r.id, BATCH).await?;
            if batch.is_empty() {
                break;
            }
            for uid in &batch {
                if backfill_one(db, r, *uid).await? {
                    granted += 1;
                }
            }
            batches += 1;
            if batches >= MAX_BATCHES {
                // 本 tick 到此为止；不标记 backfill_done，下轮继续。
                tracing::info!(medal = %r.name, "回填达批上限，余下留待下轮");
                break;
            }
        }
        // 扫完（无待授）才标记完成；达上限的不标记。
        if pending_users(db, r.id, 1).await?.is_empty() {
            sqlx::query("UPDATE medals SET backfill_done = TRUE WHERE id = $1")
                .bind(r.id)
                .execute(db)
                .await?;
            done_rules += 1;
        }
    }
    if done_rules > 0 || granted > 0 {
        tracing::info!(done_rules, granted, "勋章回填推进");
    }
    Ok((done_rules, granted))
}

/// 回填作业的返回口径包装。
///
/// `backfill` 返回 `(已完成成就数, 授予人数)`，而 `spawn_lock!` /
/// `manual` 要的是单个 `u64` 计数。取「授予人数」——那才是本轮实际
/// 产生的效果；「成就数」只是内部游标，登记进 job_status 没有运营含义。
pub(crate) async fn medal_backfill(db: &PgPool) -> anyhow::Result<u64> {
    Ok(backfill(db).await?.1)
}

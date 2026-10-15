//! threshold 模式：周期性整体重算（无增量）。

use sqlx::PgPool;

use super::{
    award, awarded, progress_of, rules_of, save_progress, RuleRow, BATCH,
};
use super::api_domain as md;

/// 阈值模式的统计值（按周期取）。
///
/// 取数口径与站内既有统计同源，避免「成就里的上传量」和「个人页显示的
/// 上传量」对不上——这类不一致一旦出现就极难解释。
async fn threshold_value(
    db: &PgPool,
    user_id: i64,
    rule: &str,
) -> i64 {
    match rule {
        "uploaded" => sqlx::query_scalar(
            "SELECT COALESCE(uploaded, 0)::bigint FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0),
        "seed_hours" => sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds), 0)::bigint / 3600 \
             FROM snatches WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0),
        "uploads" => sqlx::query_scalar(
            "SELECT count(*)::bigint FROM torrents \
             WHERE owner_id = $1 AND approval_status = 1",
        )
        .bind(user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0),
        "registered_days" => sqlx::query_scalar(
            "SELECT GREATEST(0, EXTRACT(DAY FROM now() - created_at)) \
                   ::bigint FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0),
        // 未识别的规则名 → 0（永不达成），而不是猜一个数。
        // 猜错会导致「配错一条规则就人人秒达成」。
        _ => 0,
    }
}

/// 给一个人重算某个 threshold 成就。返回是否新授予。
pub(super) async fn eval_threshold_one(
    db: &PgPool,
    r: &RuleRow,
    user_id: i64,
) -> anyhow::Result<bool> {
    let rule = md::Rule {
        mode: md::Mode::Threshold,
        period: md::Period::parse(r.auto_period.as_deref()),
        target: r.auto_target.unwrap_or(0),
    };
    let cur =
        threshold_value(db, user_id, r.auto_rule.as_deref().unwrap_or(""))
            .await;
    let done = awarded(db, user_id, r.id).await;
    match md::judge_threshold(&rule, cur, done) {
        md::Verdict::Award => award(db, user_id, r, "threshold").await,
        md::Verdict::Progressed { current } => {
            save_progress(db, user_id, r.id, current).await?;
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// threshold 模式的周期扫描（小时级 tick 调用）。
pub(crate) async fn threshold_sweep(db: &PgPool) -> anyhow::Result<u64> {
    let rules = rules_of(db, Some("threshold")).await?;
    let mut granted = 0u64;
    for r in &rules {
        // 每个成就只扫「还没拿到的人」——已授予的不必再算。
        let candidates = super::pending_users(db, r.id, BATCH).await?;
        for uid in candidates {
            if eval_threshold_one(db, r, uid).await? {
                granted += 1;
            }
        }
    }
    if granted > 0 {
        tracing::info!(granted, "threshold 成就扫描授予");
    }
    Ok(granted)
}

/// event 模式回填时的当前累计（供 backfill 调用）。
pub(super) async fn event_total(
    db: &PgPool,
    user_id: i64,
    event: &str,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM medal_events \
         WHERE user_id = $1 AND event = $2",
    )
    .bind(user_id)
    .bind(event)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

/// 读进度（转发出给 backfill / event 用）。
pub(crate) async fn prog(
    db: &PgPool,
    user_id: i64,
    medal_id: i64,
) -> i64 {
    progress_of(db, user_id, medal_id).await
}

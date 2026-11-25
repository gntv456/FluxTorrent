//! tick 任务组（自 run.rs 拆出守 300 行门禁）：60s 常规组 + 5min 重巡组。
//! 宏 `spawn_lock!` 从 run.rs 文本级导出（`pub(crate) use spawn_lock;`）。

use super::run::{job_in_shard, spawn_lock};
use super::*;
use sqlx::PgPool;

/// 60s 常规任务组（自 run 循环抽出守行数/行宽门禁；宏先定义后用）。
pub(super) fn reg_60s(db: &PgPool, shard: &Option<Vec<String>>) {
    let (db2, shard2) = (db.clone(), shard.clone());
    tokio::spawn(async move {
        let mut js = tokio::task::JoinSet::new();
        spawn_lock!(
            js,
            &db2,
            "job:backfill_pieces_hash",
            backfill_pieces_hash,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:sweep_stale_peers",
            sweep_stale_peers,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:task_settle",
            crate::task_jobs::task_settle,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:exam_assign",
            crate::task_jobs::exam_assign,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:lottery_settle",
            lottery_settle_due,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:flx_settle",
            fixed_lottery_settle_due,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:flx_fulfill",
            fixed_lottery_fulfill_due,
            &shard2
        );
        while let Some(res) = js.join_next().await {
            let _ = res.map_err(|e| tracing::error!(?e, "j"));
        }
    });
}

/// 5 分钟重型巡检任务组（hr 三件 + 里程碑 + 分级 + 保种退出）。
pub(super) fn reg_5min(db: &PgPool, shard: &Option<Vec<String>>) {
    let (db2, shard2) = (db.clone(), shard.clone());
    tokio::spawn(async move {
        let mut js = tokio::task::JoinSet::new();
        spawn_lock!(js, &db2, "job:hr_enforce", hr_enforce, &shard2);
        spawn_lock!(
            js,
            &db2,
            "job:hr_warn_ladder",
            hr_warn_ladder::run,
            &shard2
        );
        spawn_lock!(js, &db2, "job:hr_punish", hr_punish, &shard2);
        spawn_lock!(
            js,
            &db2,
            "job:collect_milestones",
            collect_milestones,
            &shard2
        );
        spawn_lock!(
            js,
            &db2,
            "job:class_auto_adjust",
            class_auto_adjust,
            &shard2
        );
        spawn_lock!(js, &db2, "job:preserve_seed", preserve_seed, &shard2);
        while let Some(res) = js.join_next().await {
            let _ = res.map_err(|e| tracing::error!(?e, "j"));
        }
    });
}

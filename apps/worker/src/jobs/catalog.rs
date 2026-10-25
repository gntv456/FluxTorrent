//! 任务目录同步（0218 G7）：启动时把 job_status 同步成目录，
//! 供后台面板展示（「Cron 可视」的唯一来源）。
//! 目录条目本身在 catalog_jobs.rs（300 行门禁：两件事别塞一个文件）。
//! ⚠️ 与 run.rs 的 tick 分组一一对应：增删 job、改节奏时两处必须同步
//! （同步后旧目录行会被删掉，面板不会留死任务按钮）。

use sqlx::PgPool;

pub struct JobDef {
    pub name: &'static str,
    /// 调度节奏：60s/10m/30m/1h/6h/24h（bank_daily 挂在 60s tick 上按日判定）
    pub cadence: &'static str,
}

pub(crate) use super::catalog_jobs::JOBS;

/// 目录同步：幂等 upsert 节奏；代码里已下线的 job 从目录里删掉（避免面板留死按钮）。
pub(crate) async fn sync_job_catalog(db: &PgPool) {
    for j in JOBS {
        let _ = sqlx::query(
            "INSERT INTO job_status (job, cadence) VALUES ($1, $2) \
             ON CONFLICT (job) DO UPDATE SET cadence = EXCLUDED.cadence, \
             updated_at = now()",
        )
        .bind(j.name)
        .bind(j.cadence)
        .execute(db)
        .await;
    }
    let known: Vec<String> = sqlx::query_scalar("SELECT job FROM job_status")
        .fetch_all(db)
        .await
        .unwrap_or_default();
    for name in known {
        if !JOBS.iter().any(|j| j.name == name) {
            let _ = sqlx::query("DELETE FROM job_status WHERE job = $1")
                .bind(&name)
                .execute(db)
                .await;
        }
    }
}

//! 任务目录（0218 G7）：全量 job 名 + 调度节奏，启动时同步进 job_status 供后台
//! 面板展示（「Cron 可视」的唯一来源）。
//! ⚠️ 与 run.rs 的 tick 分组一一对应：增删 job、改节奏时两处必须同步
//! （同步后旧目录行会被删掉，面板不会留死任务按钮）。

use sqlx::PgPool;

pub struct JobDef {
    pub name: &'static str,
    /// 调度节奏：60s/10m/30m/1h/6h/24h（bank_daily 挂在 60s tick 上按日判定）
    pub cadence: &'static str,
}

pub const JOBS: &[JobDef] = &[
    JobDef {
        name: "expire_promotions",
        cadence: "60s",
    },
    JobDef {
        name: "magic_pool_promo",
        cadence: "60s",
    },
    JobDef {
        name: "preserve_exit",
        cadence: "60s",
    },
    JobDef {
        name: "consume_announce",
        cadence: "60s",
    },
    JobDef {
        name: "consume_agent_blocks",
        cadence: "60s",
    },
    JobDef {
        name: "backfill_pieces_hash",
        cadence: "60s",
    },
    JobDef {
        name: "sweep_stale_peers",
        cadence: "60s",
    },
    JobDef {
        name: "collect_milestones",
        cadence: "60s",
    },
    JobDef {
        name: "hr_enforce",
        cadence: "60s",
    },
    JobDef {
        name: "hr_punish",
        cadence: "60s",
    },
    JobDef {
        name: "class_auto_adjust",
        cadence: "60s",
    },
    JobDef {
        name: "preserve_seed",
        cadence: "60s",
    },
    JobDef {
        name: "task_settle",
        cadence: "60s",
    },
    JobDef {
        name: "exam_assign",
        cadence: "60s",
    },
    JobDef {
        name: "lottery_settle",
        cadence: "60s",
    },
    JobDef {
        name: "bank_daily",
        cadence: "60s",
    },
    JobDef {
        name: "seeding_reward",
        cadence: "1h",
    },
    JobDef {
        name: "purge_old_login_events",
        cadence: "1h",
    },
    JobDef {
        name: "ratio_watch",
        cadence: "1h",
    },
    JobDef {
        name: "dormant_mark",
        cadence: "1h",
    },
    JobDef {
        name: "wishlist_notify",
        cadence: "1h",
    },
    JobDef {
        name: "highspeed_tag",
        cadence: "1h",
    },
    JobDef {
        name: "resurrection_settle",
        cadence: "1h",
    },
    JobDef {
        name: "social_team_settle",
        cadence: "1h",
    },
    JobDef {
        name: "social_team_expire",
        cadence: "1h",
    },
    JobDef {
        name: "jixiao_settle",
        cadence: "1h",
    },
    JobDef {
        name: "preserve_settle",
        cadence: "1h",
    },
    JobDef {
        name: "funding_settle",
        cadence: "1h",
    },
    JobDef {
        name: "refundable_settle",
        cadence: "1h",
    },
    JobDef {
        name: "achievement_grant",
        cadence: "1h",
    },
    JobDef {
        name: "subreq_sweep",
        cadence: "1h",
    },
    JobDef {
        name: "subawards",
        cadence: "1h",
    },
    JobDef {
        name: "subcert_sweep",
        cadence: "1h",
    },
    JobDef {
        name: "expire_invites",
        cadence: "1h",
    },
    JobDef {
        name: "purge_expired_tokens",
        cadence: "1h",
    },
    JobDef {
        name: "dlq_watch",
        cadence: "1h",
    },
    JobDef {
        name: "purge_runtime_logs",
        cadence: "1h",
    },
    JobDef {
        name: "cheat_audit",
        cadence: "10m",
    },
    JobDef {
        name: "multi_ip_check",
        cadence: "30m",
    },
    JobDef {
        name: "leak_scan",
        cadence: "30m",
    },
    JobDef {
        name: "reconcile_diff_alert",
        cadence: "6h",
    },
    JobDef {
        name: "reconcile_snapshots",
        cadence: "6h",
    },
    JobDef {
        name: "ensure_partitions",
        cadence: "24h",
    },
];

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

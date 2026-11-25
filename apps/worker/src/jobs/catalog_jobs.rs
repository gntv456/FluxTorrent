//! 任务目录清单（0218 G7）：全量 job 名 + 调度节奏。
//!
//! 自 catalog.rs 拆出（300 行门禁）：目录条目本身只是数据，
//! 与「同步进 job_status」的逻辑放一起会让两个文件都超限。
//! ⚠️ 与 run.rs 的 tick 分组一一对应：增删 job、改节奏时两处必须同步。

use super::catalog::JobDef;

pub use super::catalog_jobs_slow::JOBS_SLOW;

/// 60s 快组（每 tick 主力）。
pub const JOBS_FAST: &[JobDef] = &[
    JobDef {
        name: "expire_promotions",
        cadence: "60s",
    },
    // run.rs:164 一直在调度它，但 catalog 里从来没登记过 ⇒ 面板看不见、
    // 不显示 cadence、也不能手点（ND3 实测抓到的幽灵调度）。
    JobDef {
        name: "announce_site_promos",
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
    // 2026-10-29：60s tick 在跑但目录没登记 ⇒ 补登记（P-471 同族）
    JobDef {
        name: "consume_xreport",
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
        name: "hr_warn_ladder",
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
        name: "mail_outbox",
        cadence: "60s",
    },
    JobDef {
        name: "spark_merkle",
        cadence: "60s",
    },
    JobDef {
        name: "flx_settle",
        cadence: "60s",
    },
    JobDef {
        name: "flx_fulfill",
        cadence: "60s",
    },
    JobDef {
        // 领取期限扫：挂 flx_fulfill 同一 60s tick（manual 可手触）
        name: "flx_claim_expire",
        cadence: "60s",
    },
];

/// 全量遍历：快组在前，慢组在后（顺序无关）。
pub const JOBS_GROUPS: [&[JobDef]; 2] = [JOBS_FAST, JOBS_SLOW];

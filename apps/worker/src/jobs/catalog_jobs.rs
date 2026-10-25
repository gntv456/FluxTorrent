//! 任务目录清单（0218 G7）：全量 job 名 + 调度节奏。
//!
//! 自 catalog.rs 拆出（300 行门禁）：目录条目本身只是数据，
//! 与「同步进 job_status」的逻辑放一起会让两个文件都超限。
//! ⚠️ 与 run.rs 的 tick 分组一一对应：增删 job、改节奏时两处必须同步。

use super::catalog::JobDef;

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
        name: "risk_backtest",
        cadence: "24h",
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
        // 0404 §17-2：run.rs 的 60s tick 一直在跑它，但目录里没有，
        // 于是后台面板看不见、也不能手触——补登记。
        name: "mail_outbox",
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
        // 0295（运营轮 P1-6）：run.rs 的 1h tick 一直在跑它，但目录里没有，
        // 于是后台面板看不见、也不能手触——补登记。
        name: "request_expire",
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
        name: "game_coupons",
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
        name: "expire_medals",
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
        // 0357 审计日志归档：run.rs hour_tick 调度，Redis 键日节流
        name: "audit_retain",
        cadence: "1h",
    },
    JobDef {
        name: "cheat_audit",
        cadence: "10m",
    },
    JobDef {
        name: "cheat_enforce",
        cadence: "10m",
    },
    JobDef {
        // 0368 M2 风险评分：增量轮（脏用户：新证据/新处置/1h 未重算）
        name: "risk_score",
        cadence: "10m",
    },
    JobDef {
        // 0368 M2 风险评分：每日全量（权重调参后的安全网）
        name: "risk_score_daily",
        cadence: "24h",
    },
    JobDef {
        // 0370 M4 注册治理：过期同 IP 注册摘要清理（仅存 sha3）
        name: "reg_cleanup",
        cadence: "24h",
    },
    JobDef {
        name: "collusion_check",
        cadence: "10m",
    },
    JobDef {
        // 0400 自动审种引擎轮：run.rs 的 10m tick 一直在跑它，但目录里没有，
        // 于是后台面板看不见、也不能手触——补登记。
        name: "auto_review",
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
    JobDef {
        name: "usage_stats",
        cadence: "24h",
    },
    JobDef {
        // 0416 每日火花流水 Merkle 封存（独立 60s 循环，体内幂等）
        name: "spark_merkle",
        cadence: "60s",
    },
    JobDef {
        // 0400 §4.3 自动审种每日摘要：run.rs 的 1d tick 在跑，目录里没有——补登记。
        name: "auto_review_digest",
        cadence: "24h",
    },
    // 0382 勋章自动成就：threshold 周期重算（hourly 而非 daily——周期口径
    // 含日/周，daily 会让「周期最后一天达标的用户」白等 24h）
    JobDef {
        name: "medal_threshold",
        cadence: "1h",
    },
    // 0382 存量回填：分批续跑，靠「已授予即跳过」续档
    JobDef {
        name: "medal_backfill",
        cadence: "1h",
    },
    // 0383 商城履约自愈：把卡在 frozen 且过了宽限的订单推到终态
    JobDef {
        name: "shop_fulfill_recover",
        cadence: "1h",
    },
    // 0382 做种事件桥（hourly 而非 announce 热路径）
    JobDef {
        name: "medal_seeding_bridge",
        cadence: "1h",
    },
];

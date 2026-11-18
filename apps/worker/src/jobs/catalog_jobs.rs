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
    // run.rs:164 一直在调度它，但 catalog 里从来没登记过 ⇒ 面板看不见、
    // 不显示 cadence、也不能手点（ND3 实测抓到的幽灵调度）。
    JobDef {
        name: "announce_site_promos",
        cadence: "60s",
    },
    // P-471 同族：tick1d 在跑但目录未登记 ⇒ 面板看不见（0424 月结）
    JobDef {
        name: "contract_settle",
        cadence: "24h",
    },
    // 0496 契约采样：hour_jobs 在跑，目录+手触同步登记
    JobDef {
        name: "contract_sample",
        cadence: "1h",
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
        // 0404 §17-2：60s tick 在跑但目录没有 ⇒ 补登记
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
        // 0295 P1-6：1h tick 在跑但目录没有 ⇒ 补登记
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
        // 0400：10m tick 在跑但目录没有 ⇒ 补登记
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
    JobDef {
        // 0487 IYUU 辅种查重：hour_jobs 每小时在跑，目录/手触此前都缺——
        // 站长配好凭据后可在面板立即跑一轮，不用等整点。
        name: "iyuu_refresh",
        cadence: "1h",
    },
    JobDef {
        // 0487 配套：删种后的辅种缓存死行清理，run.rs 日频兜底在跑——
        // catalog 未登记会让面板看不见也不能手动补跑（catalog guard 报红）。
        name: "iyuu_orphan_sweep",
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

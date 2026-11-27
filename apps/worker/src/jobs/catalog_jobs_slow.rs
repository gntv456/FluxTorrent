//! 1h/24h 慢组任务目录（自 catalog_jobs.rs 拆出守 300 行门禁）。
//! ⚠️ 与 run.rs / run_tick_groups.rs 的 tick 分组一一对应：增删 job、
//! 改节奏时两处必须同步。

use super::catalog::JobDef;

/// 1h/24h 慢组。
pub const JOBS_SLOW: &[JobDef] = &[
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
    // 0503 新人生命周期跟进（day-3 挽留 + 首做种祝贺）
    JobDef {
        name: "onboarding_followup",
        cadence: "1h",
    },
    // 0503 竞赛结算：到期赛发名次奖（上传竞速口径，hourly 幂等）
    JobDef {
        name: "contest_settle",
        cadence: "1h",
    },
    // 0503 流失梯度 30/60 天 + 注册周年 PM
    JobDef {
        name: "lifecycle_touch",
        cadence: "1h",
    },
    // 0503 娱乐屋周榜派奖（上周 TOP3，幂等键防重）
    JobDef {
        name: "arcade_week_prize",
        cadence: "1h",
    },
    JobDef {
        name: "risk_backtest",
        cadence: "24h",
    },
    JobDef {
        name: "seeding_reward",
        cadence: "1h",
    },
    JobDef {
        name: "traffic_reward",
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
        name: "preserve_free_promo",
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
        name: "risk_score",
        cadence: "10m",
    },
    JobDef {
        name: "risk_score_daily",
        cadence: "24h",
    },
    JobDef {
        name: "reg_cleanup",
        cadence: "24h",
    },
    JobDef {
        name: "collusion_check",
        cadence: "10m",
    },
    JobDef {
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
        name: "auto_review_digest",
        cadence: "24h",
    },
    JobDef {
        name: "iyuu_refresh",
        cadence: "1h",
    },
    JobDef {
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

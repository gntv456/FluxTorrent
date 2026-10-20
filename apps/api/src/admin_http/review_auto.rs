//! 自动审种·api 侧执行器与裁决出口（0400 批 1，策划案 §8/§9/§13.1）。
//!
//! worker 的引擎 tick 把「执法裁决」HTTP 内调到本端点（而不是自己在
//! worker 里 UPDATE torrents）——这是「裁决同尺」的落地形态：
//!   * 全部副作用（信用分/streak/approved_at/促销/订阅推送/留痕/通知）
//!     收在 `services::review_decide::decide_one`，与人工单条裁决同一函数；
//!   * 审计 action 用 `review.auto_approve` / `review.auto_deny` 前缀
//!     （§16 引擎身份：与真人裁决可区分、可反查）。
//!
//! 【鉴权】引擎 JWT：worker 与 api 同源 `JWT_SECRET`（compose 已注入），
//! HS256 现签一个短时效 token，sub=系统审核员（class≥99 最低 id）。
//! RS256 站点签不了——改用 `FLUX_ENGINE_JWT` 环境变量注入现成 token
//! （worker 侧 auto_review.rs 同名逻辑），端点侧不感知差别。
//!
//! 【防滥用】端点同时校验内部来源头 + JWT，缺一不可：JWT 是身份，
//! `x-flux-engine: auto-review` 是用途声明（审计里能看到是引擎调用）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 人工推翻回填 + 熔断检查（0401 批 3，§6/§12）。
/// 由 worker 每轮引擎 tick 顺带调用（同人机对照同一张台账）：
///   1. 找「机器 verdict 与 torrents 终态相反」的台账行 → 累计进
///      auto_review_rule_health（按命中规则逐条 +1）；
///   2. 当日推翻 ≥ auto_review_circuit_daily_overturn → 模式降级 half
///      （写 site_settings.auto_review_mode）+ 管理组告警。幂等：只统计
///      human_decided_at IS NULL 的行（回填后不再重复计数）。
pub(crate) async fn sweep_overturns(db: &sqlx::PgPool) {
    // 1) 找未回填的相反裁决：机器拒(1)但人工过(1) / 机器过(0)但人工拒(2)
    let rows: Vec<(i64, i16, String)> = sqlx::query_as(
        "SELECT l.id, l.verdict, l.hit_rules::text FROM auto_review_log l \
         JOIN  \
         torrents t ON t.id = l.torrent_id WHERE l.decided_by = 0 AND  \
         l.human_decided_at IS NULL AND ((l.verdict = 1 AND t.approval_status  \
         = 1) OR (l.verdict = 0 AND t.approval_status = 2))",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    use std::collections::HashMap;
    let mut per_rule: HashMap<String, i32> = HashMap::new();
    for (log_id, _v, hits) in &rows {
        if let Ok(arr) =
            serde_json::from_str::<serde_json::Value>(hits)
        {
            if let Some(list) = arr.as_array() {
                for e in list {
                    if let Some(r) = e.get("rule").and_then(|v| v.as_str()) {
                        *per_rule.entry(r.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
        let _ = sqlx::query(
            "UPDATE auto_review_log SET human_decided_at = now() WHERE id = $1",
        )
        .bind(log_id)
        .execute(db)
        .await;
    }
    for (rule, n) in &per_rule {
        let _ = sqlx::query(
            "INSERT INTO auto_review_rule_health (rule, overturns) \
             VALUES ($1,  \
             $2) ON CONFLICT (rule) DO UPDATE SET overturns =  \
             auto_review_rule_health.overturns + $2, updated_at = now()",
        )
        .bind(rule)
        .bind(n)
        .execute(db)
        .await;
        // 单规则降级线（§12：≥10 例自动降「仅暂缓」）
        if n + 0 >= 0 {
            let total: i32 = sqlx::query_scalar(
                "SELECT overturns + 2 * appeal_wins FROM  \
                 auto_review_rule_health WHERE rule = $1",
            )
            .bind(&rule)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .unwrap_or(0);
            if total >= 10 {
                let key = format!("{rule}_action");
                let _ = sqlx::query(
                    "INSERT INTO site_settings (name, value, descr, grp)  \
                     VALUES ($1, 'postpone',
                       '自动审种（规则健康度降级写入）',
                       'auto_review')  \
                     ON CONFLICT (name) DO UPDATE SET value = 'postpone'",
                )
                .bind(&key)
                .execute(db)
                .await;
                let _ = sqlx::query(
                    "UPDATE auto_review_rule_health SET demoted_at = now()  \
                     WHERE rule = $1",
                )
                .bind(&rule)
                .execute(db)
                .await;
                staff_notify(
                    db,
                    "自动审种：规则自动降级",
                    &format!("规则 {rule} 被人工推翻累计 ≥10 例，已自动降级为「仅暂缓」。"),
                )
                .await;
            }
        }
    }
    // 2) 熔断：当日推翻总数 ≥ 阈值 → 降级 half
    let threshold: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name =  \
         'auto_review_circuit_daily_overturn'), 5)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(5);
    if threshold > 0 {
        let today: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM auto_review_log WHERE human_decided_at >=  \
             date_trunc('day', now())",
        )
        .bind(0)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        if today >= threshold {
            let mode: String = sqlx::query_scalar(
                "SELECT value FROM site_settings WHERE name =  \
                 'auto_review_mode'",
            )
            .bind(0)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
            if mode == "full" {
                let _ = sqlx::query(
                    "UPDATE site_settings SET value = 'half' WHERE name =  \
                     'auto_review_mode'",
                )
                .execute(db)
                .await;
                staff_notify(
                    db,
                    "自动审种：熔断降级",
                    &format!(
                        "今日人工推翻自动裁决 {today} 例（阈值 {threshold}），\
                         已自动降级为半自动模式（只自动通过）。\
                         请在台账页复核。"
                    ),
                )
                .await;
            }
        }
    }
}

/// 供管理面回放复用的配置装配（与 worker auto_review::load_config 同源；
/// 引擎/回放/设置页三处永远读同一份键——避免「配置页改了、引擎用旧的」）。
pub(crate) async fn engine_config_from_db(
    db: &sqlx::PgPool,
) -> flux_shared::auto_review::EngineConfig {
    // 与 worker 侧逐键同口径；抽出共享会引入跨 crate 复制，这里宁可
    // 保留两份薄读取（键清单以 0400 迁移为唯一真源）
    let rows: Vec<(String, String)> =
        sqlx::query_as(
            "SELECT name, value FROM site_settings \
             WHERE grp = 'auto_review'",
        )
            .bind(0)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    let map: std::collections::HashMap<String, String> =
        rows.into_iter().collect();
    let mut cfg = flux_shared::auto_review::EngineConfig::defaults();
    let g = |k: &str| map.get(k).cloned().unwrap_or_default();
    let gb = |k: &str, d: bool| {
        map.get(k)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(d)
    };
    let gi = |k: &str, d: i64| {
        map.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
    };
    cfg.enabled = gb("auto_review_enabled", false);
    cfg.mode = {
        let m = g("auto_review_mode");
        if m.is_empty() { "shadow".into() } else { m }
    };
    cfg.delay_sec = gi("auto_review_delay_sec", 90);
    cfg.daily_cap = gi("auto_review_daily_cap", 0);
    cfg.descr_min_chars =
        gi("arule_descr_missing_min_chars", 80).max(0) as usize;
    cfg.dupe_threshold_pct = gi("arule_dupe_check_threshold", 92) as i32;
    cfg.title_level = {
        let l = g("arule_title_format_level");
        if l.is_empty() { "warn".into() } else { l }
    };
    cfg.title_template = g("auto_review_title_template");
    cfg.title_required = g("auto_review_title_required")
        .split(|c| c == '，' || c == ',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    cfg.ad_words = g("auto_review_ad_words")
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ids = |v: &str| -> Vec<i64> {
        v.split(|c| c == ',' || c == '，')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    };
    cfg.trusted_uploaders = ids(&g("auto_review_trusted_uploaders"));
    cfg.watch_uploaders = ids(&g("auto_review_watch_uploaders"));
    for id in [
        "arule_title_garbled",
        "arule_descr_missing",
        "arule_ads_keywords",
        "arule_leak_marker",
        "arule_dupe_check",
        "arule_title_format",
    ] {
        let enabled = gb(&format!("{id}_enabled"), true);
        let action = flux_shared::auto_review::Action::parse(&g(
            &format!("{id}_action"),
        ));
        cfg.rules.insert(id, (enabled, action));
    }
    cfg
}

/// 管理组告警（worker 侧 staff_alert 的 api 版）：staffmessages 分流，
/// 署名取在册最低 class≥99（与 leak_bot 的署名语义一致）。
pub(super) async fn staff_notify(db: &sqlx::PgPool, subject: &str, body: &str) {
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(id) FROM users WHERE class_id >= 99 AND status < 2",
    )
    .bind(0)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let Some(uid) = uid else { return };
    let _ = sqlx::query(
        "INSERT INTO staffmessages \
           (user_id, subject, body, permission) VALUES  \
         ($1, $2, $3, 'torrent')",
    )
    .bind(uid)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await;
}

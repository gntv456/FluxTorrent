//! 反作弊风险评分（0368 M2，对标 PT Studio FR-ACH-010 简化版）。
//!
//! 回答「这个用户总体多可疑」，给案件队列排序与观察名单供料：
//!   score = min(100, round( Σ_家族 w × f(hits) × decay(days) × 100 / NORM ))
//!   f(hits) = 1 + log2(min(hits,16))；decay = 2^(−days/半衰期)
//! 家族 = agent 冒号前缀（deal: → self_deal；down_under/bitthief 无前缀）；
//! 权重/阈值/NORM/半衰期全部 site_settings 化（risk_* 键，0368 种子）。
//! 分段 none/low/watch/high/critical；watch+ 进观察名单（staff 可见，
//! 用户无感）；high/critical 且无进行中案件 → 自动开案（priority 2/1，
//! 复用 cheat_enforce::auto_open_case，与 M1 的 L2 路径共用挡重索引）。
//!
//! 两种轮次（catalog 两个条目，面板均可手触）：
//!   risk_score（10m 增量）：脏用户 = 无分数行 / 有新证据(last_seen) /
//!     有新处置(resolved_at) / 分数行超过 1h 未重算（decay 漂移兜底）。
//!     最后一条是关键：衰减必须「无新证据也持续生效」，否则分数会冻结
//!     （验收项「30 天无新证据衰减过半」依赖它）。
//!   risk_score_daily（24h 全量）：近 90 天有证据的用户 ∪ 全部分数行，
//!     权重调参后的安全网（增量轮 1h 内也会自愈，双保险）。
//!
//! 只计入 resolved_at IS NULL 的存活证据（staff 处置即解除）；职员
//! （class_id ≥ 90）不计分——其测试/运维流量不生成观察名单。
//! 已处置/过期用户的分数行**不删**（保留 0 分留存），删了会被增量轮
//! 的「无分数行」分支立刻插回，造成删-插 churn。
//!
//! 写入门禁：upsert 的 WHERE 只挡 score/band 均未变——这是 decay 机制的
//! 支点（未变则不写 ⇒ updated_at 停留 ⇒ 1h 后落入脏集被重算）。若去掉
//! 门禁变成每轮必写，updated_at 永远新鲜，无新证据的用户再也不进脏集，
//! 分数冻结。shadow_breakdown 随分数变化时刷新（含各家族 hits/权重/贡献分）。

use super::cheat_enforce::auto_open_case;
use super::risk_backtest::drain_pending_backtests;
use super::staff_alert::{alerted_within_7d, staff_alert};
use sqlx::PgPool;
use std::collections::HashMap;

/// 证据窗口：decay(90d, 半衰期 14d) ≈ 0.0116，满配家族（12.5 分）此时
/// 仅贡献 ≈0.14 分——更早的证据对分数已无实际影响，截窗省全表扫描。
const RECENT_DAYS: i32 = 90;
/// 规则集版本（0391）。
///
/// 此前是**硬编码常量**：改公式或调权后，存量分数行的版本号还是 1，
/// 面板上「这批分数是哪版权重算的」无从判断。现改为读
/// `cheat_rulesets.active`——分数行指向一张真实存在的版本，
/// 任何时候都能查回当时的权重快照。
///
/// 查不到（例如 0391 迁移尚未落库）时退回 1，保持旧行为。
async fn active_ruleset_ver(db: &PgPool) -> i32 {
    sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(max(version), 1) FROM cheat_rulesets WHERE active",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(1)
}

/// 影子模式：1 = 新版本只算分不开案（运营对比无回归后再关）。
async fn shadow_mode(db: &PgPool) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'risk_shadow_mode'), '1')",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "0" && v != "no")
    .unwrap_or(true)
}
/// 档位广播去重窗（同 staff_alert 的 7 天口径）。
const BAND_SUBJ: &str = "风险评分档位变化";

/// 公式参数容器：0368 种子的 risk_* 键一网打尽，缺行回落出厂值。
pub(super) struct RiskCfg {
    pub(super) kv: HashMap<String, f64>,
}

impl RiskCfg {
    pub(super) fn f(&self, key: &str, default: f64) -> f64 {
        self.kv.get(key).copied().unwrap_or(default)
    }
    /// 家族权重：未列名家族落 risk_weight_default；负权当 0（防呆）。
    pub(super) fn weight(&self, family: &str) -> f64 {
        let dflt = self.f("risk_weight_default", 0.5).max(0.0);
        self.f(&format!("risk_weight_{family}"), dflt).max(0.0)
    }
    pub(super) fn band(&self, score: i32) -> &'static str {
        let s = score as f64;
        if s >= self.f("risk_band_critical", 80.0) {
            "critical"
        } else if s >= self.f("risk_band_high", 60.0) {
            "high"
        } else if s >= self.f("risk_band_watch", 40.0) {
            "watch"
        } else if s >= self.f("risk_band_low", 20.0) {
            "low"
        } else {
            "none"
        }
    }
}

/// 一次聚合行：某用户某家族的存活证据画像。
pub(super) struct Fam {
    pub(super) family: String,
    pub(super) hits: i64,
    pub(super) days: f64,
}

/// 读全部 risk_* 设置（一查询；解析失败的键当缺省）。
async fn risk_cfg(db: &PgPool) -> anyhow::Result<RiskCfg> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name LIKE 'risk\\_%'",
    )
    .fetch_all(db)
    .await?;
    let kv = rows
        .into_iter()
        .filter_map(|(k, v)| v.trim().parse::<f64>().ok().map(|n| (k, n)))
        .collect();
    Ok(RiskCfg { kv })
}

/// 10m tick 增量轮（run.rs spawn_lock 只传单参，包一层）。
/// 顺带消费已排队的回测占位（见 risk_backtest.rs）。
pub async fn risk_score_tick(db: &PgPool) -> anyhow::Result<u64> {
    let mut written = risk_score(db, false).await?;
    written += drain_pending_backtests(db).await;
    Ok(written)
}

/// 24h tick 全量轮。
pub async fn risk_score_daily(db: &PgPool) -> anyhow::Result<u64> {
    risk_score(db, true).await
}

/// 评分主流程：脏用户聚合 → Rust 侧算分 → upsert（挡未变）→ 档位广播
/// → high/critical 自动开案。返回本轮实际改写（分数/档位变化）的行数。
pub async fn risk_score(db: &PgPool, full: bool) -> anyhow::Result<u64> {
    let cfg = risk_cfg(db).await?;
    let ruleset_ver = active_ruleset_ver(db).await;
    let shadow = shadow_mode(db).await;
    let norm = cfg.f("risk_norm", 8.0).max(0.5);
    let half = cfg.f("risk_decay_days", 14.0).clamp(0.5, 3650.0);

    // 脏用户集（增量/全量两种口径）+ 同查询内按家族聚合。
    // LEFT JOIN cheat_events：脏集中无存活证据的用户也出一行
    // （family='default', hits=0）——这是清零路径，不能漏。
    // users 内连接（class<90, status<2）：职员/墓碑号不评分。
    let dirty = if full {
        "SELECT c.user_id AS uid FROM cheat_events c \
         WHERE c.last_seen > now() - make_interval(days => $1::int) \
         UNION SELECT r.user_id FROM user_risk_scores r"
    } else {
        "SELECT c.user_id AS uid FROM cheat_events c \
         LEFT JOIN user_risk_scores r ON r.user_id = c.user_id \
         WHERE c.last_seen > now() - make_interval(days => $1::int) \
           AND (r.user_id IS NULL OR c.last_seen > r.updated_at \
                OR c.resolved_at > r.updated_at) \
         UNION \
         SELECT r.user_id FROM user_risk_scores r \
         WHERE r.updated_at < now() - interval '1 hour'"
    };
    let sql = format!(
        "WITH dirty AS ({dirty}) \
         SELECT d.uid, \
                CASE \
                  WHEN c.agent LIKE 'ghost:%' THEN 'ghost' \
                  WHEN c.agent LIKE 'speed:%' THEN 'speed' \
                  WHEN c.agent LIKE 'reset:%' THEN 'reset' \
                  WHEN c.agent LIKE 'corr:%' THEN 'corr' \
                  WHEN c.agent LIKE 'xreport_over:%' THEN 'xreport_over' \
                  WHEN c.agent LIKE 'nearcap:%' THEN 'nearcap' \
                  WHEN c.agent LIKE 'deal:%' THEN 'self_deal' \
                  WHEN c.agent = 'down_under' THEN 'down_under' \
                  WHEN c.agent = 'bitthief' THEN 'bitthief' \
                  ELSE 'default' \
                END AS family, \
                COALESCE(SUM(c.hits), 0)::bigint AS hits, \
                COALESCE(EXTRACT(EPOCH FROM (now() - MAX(c.last_seen))) \
                   ::float8 / 86400.0, 0.0) AS days \
         FROM dirty d \
         JOIN users u ON u.id = d.uid \
         LEFT JOIN cheat_events c ON c.user_id = d.uid \
           AND c.resolved_at IS NULL \
           AND c.last_seen > now() - make_interval(days => {RECENT_DAYS}) \
         WHERE u.class_id < 90 AND u.status < 2 \
         GROUP BY d.uid, family"
    );
    let rows: Vec<(i64, String, i64, f64)> =
        sqlx::query_as(&sql).bind(RECENT_DAYS).fetch_all(db).await?;
    if rows.is_empty() {
        return Ok(0);
    }

    // 旧档位（广播比较用；全表一把捞，表只含有过证据的用户，量小）
    let old_bands: HashMap<i64, String> =
        sqlx::query_as("SELECT user_id, band FROM user_risk_scores")
            .fetch_all(db)
            .await?
            .into_iter()
            .collect();

    // 按用户归集家族（hits=0 的占位行也要进 map：清零路径）
    let mut users: HashMap<i64, Vec<Fam>> = HashMap::new();
    for (uid, family, hits, days) in rows {
        let e = users.entry(uid).or_insert_with(Vec::new);
        if hits > 0 {
            e.push(Fam {
                family,
                hits,
                // 时钟回拨容错：负 days 会算出 >1 的 decay
                days: days.max(0.0),
            });
        }
    }

    let mut written = 0u64;
    for (uid, fams) in &users {
        // 公式：Σ w × (1+log2(min(hits,16))) × 2^(−days/半衰期) × 100/NORM
        let mut total = 0f64;
        let mut parts = serde_json::Map::new();
        let mut desc: Vec<String> = Vec::new();
        for f in fams {
            let w = cfg.weight(&f.family);
            let fh = 1.0 + (f.hits.min(16) as f64).log2();
            let decay = 2f64.powf(-f.days / half);
            let pts = w * fh * decay * 100.0 / norm;
            total += pts;
            parts.insert(
                f.family.clone(),
                serde_json::json!({
                    "hits": f.hits,
                    "days": (f.days * 100.0).round() / 100.0,
                    "weight": w,
                    "points": (pts * 100.0).round() / 100.0,
                }),
            );
            desc.push(format!(
                "{}×{}({:.1}分)",
                f.family,
                f.hits,
                (pts * 10.0).round() / 10.0
            ));
        }
        let score = (total.round() as i32).clamp(0, 100);
        let band = cfg.band(score);
        let breakdown = serde_json::json!({
            "families": parts,
            "norm": norm,
            "half_life_days": half,
            "ruleset_ver": ruleset_ver,
        });
        let n = sqlx::query(
            "INSERT INTO user_risk_scores \
               (user_id, score, band, shadow_breakdown, ruleset_ver, \
                updated_at) \
             VALUES ($1, $2, $3, $4, $5, now()) \
             ON CONFLICT (user_id) DO UPDATE SET score = EXCLUDED.score, \
               band = EXCLUDED.band, \
               shadow_breakdown = EXCLUDED.shadow_breakdown, \
               ruleset_ver = EXCLUDED.ruleset_ver, \
               updated_at = now() \
             WHERE user_risk_scores.score IS DISTINCT FROM EXCLUDED.score \
                OR user_risk_scores.band IS DISTINCT FROM EXCLUDED.band",
        )
        .bind(uid)
        .bind(score)
        .bind(band)
        .bind(&breakdown)
        .bind(ruleset_ver)
        .execute(db)
        .await?
        .rows_affected();
        written += n;

        // 档位广播：跨观察名单线（进出 watch+）与升入 critical 才打扰
        // staff——none↔low 太噪且无运营意义，静默。
        let old = old_bands.get(uid).map(|s| s.as_str()).unwrap_or("none");
        let watch = |b: &str| matches!(b, "watch" | "high" | "critical");
        let escalate = band == "critical" && old != "critical";
        if watch(old) != watch(band) || escalate {
            let prefix = format!("[[risk:{uid}:{band}]]");
            if !alerted_within_7d(db, "cheater", BAND_SUBJ, &prefix).await {
                let body = format!(
                    "{prefix}用户 #{uid} 风险评分 {score}（{old} → {band}）。\
                     构成：{}。watch 及以上已进后台观察名单。",
                    desc.join(" + ")
                );
                staff_alert(db, BAND_SUBJ, &body, "cheater").await;
            }
            tracing::warn!(
                user = uid,
                score,
                old,
                band,
                "risk_score: 档位变化"
            );
        }

        // high/critical 且无进行中案件 → 自动开案（priority 2/1）。
        // 每轮重算都尝试：ON CONFLICT 挡进行中案件（幂等）；案件被 done
        // 关闭后新证据/新衰减触发重算时会开新案（同 M1 语义）。
        //
        // 【0391 影子模式】risk_shadow_mode=1 时**只记不判**：
        // 把「会开案」这件事写进 cheat_shadow_scores.would_open，
        // 但不真的开案。理由：改一个权重就可能给一批人开案，
        // 没有「先看看会多判多少人」这一步，改参数等于闭眼按扳机。
        let would_open = band == "high" || band == "critical";
        if would_open {
            if shadow {
                sqlx::query(
                    "INSERT INTO cheat_shadow_scores \
                     (user_id, ruleset_ver, score, band, delta_vs_live, \
                      would_open, breakdown, updated_at) \
                     VALUES ($1, $2, $3, $4, 0, true, $5, now()) \
                     ON CONFLICT (user_id) DO UPDATE SET \
                       ruleset_ver = EXCLUDED.ruleset_ver, \
                       score = EXCLUDED.score, band = EXCLUDED.band, \
                       would_open = true, breakdown = EXCLUDED.breakdown, \
                       updated_at = now()",
                )
                .bind(uid)
                .bind(ruleset_ver)
                .bind(score)
                .bind(band)
                .bind(&breakdown)
                .execute(db)
                .await?;
                // 影子模式下一律不真正开案。
                continue;
            }
            let prio: i16 = if band == "critical" { 2 } else { 1 };
            if let Some(cid) = auto_open_case(
                db,
                *uid,
                &format!("风险分自动开案：{score} 分（{band} 档）"),
                &format!("系统自动开案：风险评分 {score} 达 {band} 档"),
                prio,
            )
            .await
            {
                tracing::warn!(
                    user = uid,
                    case = cid,
                    score,
                    band,
                    "risk_score: 自动开案"
                );
            }
        } else if shadow {
            // 掉出 high/critical 也要清影子行，否则 would_open 会永远
            // 停在一个已经过时的 true 上。
            sqlx::query(
                "INSERT INTO cheat_shadow_scores \
                 (user_id, ruleset_ver, score, band, delta_vs_live, \
                  would_open, breakdown, updated_at) \
                 VALUES ($1, $2, $3, $4, 0, false, $5, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET \
                   ruleset_ver = EXCLUDED.ruleset_ver, \
                   score = EXCLUDED.score, band = EXCLUDED.band, \
                   would_open = false, breakdown = EXCLUDED.breakdown, \
                   updated_at = now()",
            )
            .bind(uid)
            .bind(ruleset_ver)
            .bind(score)
            .bind(band)
            .bind(&breakdown)
            .execute(db)
            .await?;
        }
    }
    Ok(written)
}

/// 读某个版本的参数快照并解析成 cfg。
pub(super) async fn ruleset_params(
    db: &PgPool,
    ver: i32,
) -> anyhow::Result<Option<RiskCfg>> {
    let params: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT params FROM cheat_rulesets WHERE version = $1",
    )
    .bind(ver)
    .fetch_optional(db)
    .await?;
    let Some(p) = params else { return Ok(None) };
    let kv = p
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    v.as_str()
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .map(|n| (k.clone(), n))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(RiskCfg { kv }))
}

/// 用给定参数对某用户的家族集合打分。
pub(super) fn score_with(cfg: &RiskCfg, fams: &[Fam]) -> (i32, String) {
    let norm = cfg.f("risk_norm", 8.0).max(0.5);
    let half = cfg.f("risk_decay_days", 14.0).clamp(0.5, 3650.0);
    let total: f64 = fams
        .iter()
        .map(|f| {
            let w = cfg.weight(&f.family);
            let fh = 1.0 + ((f.hits.clamp(1, 16) as f64).log2());
            let decay = 2f64.powf(-f.days.max(0.0) / half);
            w * fh * decay * 100.0 / norm
        })
        .sum();
    let score = (total.round() as i32).clamp(0, 100);
    (score, cfg.band(score).to_string())
}

//! M21 绩效考核：指标类型/我的绩效/指标计算/奖金领取/历史。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use super::jixiao_compute::compute_metrics;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ M21 绩效考核 ============
//
// 0106 重做（参考各 PT 站工作组考核口径）：
//   岗位（底薪 + min_requirements 指标）→ 管理组分配成员（admin 登记行）
//   → 周期（月）考核 → 达标判薪（worker 月末自动结算 + 用户 7 天补领窗口）
//   → 连续达标加成（每 N 达标月 +P%，N/P 走 site_settings，不再硬编码）
// 判定只认 min_requirements；metrics 字段仅作展示参考。

#[derive(sqlx::FromRow, serde::Serialize)]
struct JixiaoTypeRow {
    id: i64,
    name: String,
    base_pay: i64,
    metrics: serde_json::Value,
    min_requirements: serde_json::Value,
    #[sqlx(default)]
    description: String,
}

/// 绩效指标键白名单（防死键：旧实现 seed_days/seed_hours/seed_size_tb 全是
/// compute_metrics 不产出的死键，配了 required 永远不达标）。
/// 与 compute_metrics 产出的 key 一一对应，admin 建岗时校验。
/// 键集对齐 NP Exam 指标口径（上传/下载/平均做种/魔力/做种积分/发布/体积/操作）。
pub const JIXIAO_METRIC_KEYS: &[&str] = &[
    "uploaded",          // 上传增量（字节，当月 traffic_ledger 聚合）
    "downloaded",        // 下载增量（字节）
    "uploads",           // 发种增量（当月新发布数）
    "seeding_count",     // 当前做种种子数（现值口径）
    "seed_size",         // 当前做种体积（字节，现值口径）
    "seed_size_tb",      // 当前做种体积（TB，现值口径）
    "seed_hours",        // 当月做种时长（小时，基线差值）
    "avg_seed_hours", // 平均做种时长（小时/个：时长差值 ÷ 期内活跃种子数，NP 平均做种口径）
    "seed_days", // 当月有做种活动的天数（近似：snatches.last_seen_at 按天去重）
    "seed_points_delta", // 做种积分增量（1 积分 = 1 小时做种，与 exam 引擎 task_jobs 同源口径）
    "spark_delta",       // 火花增量（当月 spark_ledger 正向流水合计）
    "ops",               // 当月操作数（audit_log 按月）
];

/// 校验 min_requirements / metrics 里的键是否全部认识；返回首个未知键。
pub fn jixiao_unknown_metric_key(v: &serde_json::Value) -> Option<String> {
    v.as_object()?
        .keys()
        .find(|k| !JIXIAO_METRIC_KEYS.contains(&k.as_str()))
        .cloned()
}

/// 上一期 period（"2026-09" → "2026-08"；站点时区 UTC+8 不影响 YYYY-MM 纯字符串回退）
pub fn jixiao_prev_period(period: &str) -> Option<String> {
    let (y, m): (i32, u32) = {
        let mut it = period.split('-');
        let y = it.next()?.parse().ok()?;
        let m = it.next()?.parse().ok()?;
        (y, m)
    };
    if !(1..=12).contains(&m) {
        return None;
    }
    Some(if m == 1 {
        format!("{:04}-12", y - 1)
    } else {
        format!("{y:04}-{:02}", m - 1)
    })
}

#[get("/jixiao/types")]
pub(super) async fn jixiao_types(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, JixiaoTypeRow>(
        "SELECT id, name, base_pay, metrics, min_requirements, description FROM jixiao_types ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 读取整数型站点设置（缺省取 dft）
pub(super) async fn jixiao_setting_i64(
    db: &sqlx::PgPool,
    name: &str,
    dft: i64,
) -> i64 {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(dft)
}

/// 达标加成：每 N 个达标月 +P%。
/// **岗位级优先（0106 补丁）**：jixiao_types.bonus_rules 存了
/// `{months_per_step, percent_per_step}`（admin 表单「每 N 月 +M%」）——有值按岗位算；
/// 缺省/字段非法回落全站 site_settings（jixiao_bonus_months_per_step / _percent_per_step）。
pub async fn jixiao_bonus(
    db: &sqlx::PgPool,
    type_id: i64,
    base_pay: i64,
    qualified_months: i64,
) -> i64 {
    // 岗位级配置（表单写入；旧数据 bonus_rules='{}' 视为未配置）
    let rules: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT bonus_rules FROM jixiao_types WHERE id = $1",
    )
    .bind(type_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let (step, pct) =
        match rules.as_ref().and_then(|r| r.as_object()).map(|r| {
            (
                r.get("months_per_step").and_then(|v| v.as_i64()),
                r.get("percent_per_step").and_then(|v| v.as_i64()),
            )
        }) {
            Some((Some(step), Some(pct))) if step > 0 && pct >= 0 => {
                (step, pct)
            }
            _ => (
                jixiao_setting_i64(db, "jixiao_bonus_months_per_step", 3)
                    .await
                    .max(1),
                jixiao_setting_i64(db, "jixiao_bonus_percent_per_step", 10)
                    .await,
            ),
        };
    base_pay * pct / 100 * (qualified_months / step)
}

/// 我的绩效考核总览（NP jixiao.php 口径）：分配给我的岗位 + 本期实时指标 vs 要求 + 达标状态。
/// 未被分配岗位时 assigned 为空数组（前端展示引导文案，而不是让用户对着全岗位表点领取吃报错）。
#[get("/jixiao/me")]
pub(super) async fn jixiao_me(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 站点时区（UTC+8）取期号——与 admin 分配/worker 结算的期号口径一致
    let period = (chrono::Utc::now() + chrono::Duration::hours(8))
        .format("%Y-%m")
        .to_string();
    // 补领：上一期（跨月后窗口天内）未结算的登记行也返回（前端给「补领」按钮）
    let prev_period = jixiao_prev_period(&period);
    let days_into_month = {
        let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
        now_site
            .format("%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(99)
    };
    let window =
        jixiao_setting_i64(&state.repo.db, "jixiao_claim_window_days", 7).await;
    let claimable_prev = days_into_month <= window;

    // admin 分配行 = 本月岗位登记（source='admin'）；工资领取行 source 为空
    let assigned: Vec<(i64, String, String, i64, serde_json::Value, serde_json::Value, i16, i64)> =
        sqlx::query_as(
            "SELECT t.id, t.name, c.period, t.base_pay, t.metrics, t.min_requirements, c.status, \
                    (SELECT count(*) FROM jixiao_claims c2 \
                     WHERE c2.user_id = c.user_id AND c2.type_id = t.id \
                       AND c2.settled_at IS NOT NULL AND c2.status = 1) \
             FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
             WHERE c.user_id = $1 AND c.metrics_snapshot->>'source' = 'admin' \
               AND (c.period = $2 OR ($3 AND c.period = $4 AND c.settled_at IS NULL AND c.status = 0))",
        )
    .bind(auth.id)
    .bind(&period)
    .bind(claimable_prev)
    .bind(prev_period.clone().unwrap_or_default())
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let metrics = compute_metrics(&state.repo.db, auth.id, &period).await?;
    // 上期补领行的指标要按上期算（窗口内通常与本期差异很小，但口径必须对）
    let prev_metrics = if claimable_prev && prev_period.is_some() {
        compute_metrics(
            &state.repo.db,
            auth.id,
            prev_period.as_deref().unwrap(),
        )
        .await?
    } else {
        serde_json::Value::Null
    };
    // 单行设计（0106）：本期已发放 = 登记行 settled_at 非空（worker 结算或本人补领）
    let claimed: Vec<i64> = sqlx::query_scalar(
        "SELECT type_id FROM jixiao_claims \
         WHERE user_id = $1 AND period = $2 AND settled_at IS NOT NULL",
    )
    .bind(auth.id)
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 达标月数（加成的分子：历史 status=1 的已发薪期数）
    let qualified: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT type_id, count(*) FROM jixiao_claims \
         WHERE user_id = $1 AND status = 1 GROUP BY type_id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let claim_window = window;
    let items: Vec<serde_json::Value> = futures_util::future::join_all(assigned.iter().map(
        |(id, name, row_period, base_pay, _m, min_reqs, status, settled_months)| {
            let metrics = if row_period == &period {
                metrics.clone()
            } else {
                prev_metrics.clone()
            };
            let qualified = &qualified;
            let claimed = &claimed;
            let row_period = row_period.clone();
            let is_current = row_period == period;
            let db = &state.repo.db;
            async move {
                // 逐指标比对：current vs required + 是否达标
                let checks: Vec<serde_json::Value> = min_reqs
                    .as_object()
                    .map(|reqs| {
                        reqs.iter()
                            .filter(|(_, v)| v.as_i64().unwrap_or(0) > 0)
                            .map(|(k, v)| {
                                let required = v.as_i64().unwrap_or(0);
                                let actual = metrics.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                                serde_json::json!({
                                    "key": k, "required": required, "current": actual,
                                    "ok": actual >= required,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let all_ok = checks.iter().all(|c| c["ok"].as_bool().unwrap_or(false));
                let months = (*settled_months).max(
                    qualified
                        .iter()
                        .find(|(tid, _)| tid == id)
                        .map(|(_, n)| *n)
                        .unwrap_or(0),
                );
                let bonus = jixiao_bonus(db, *id, *base_pay, months).await;
                serde_json::json!({
                    "type_id": id, "name": name, "base_pay": base_pay,
                    "period": row_period,
                    // 上期补领行：本期 claimed 列表不含它（按期号查），永远 false 由 claimable 表达
                    "claimable_prev": !is_current,
                    "min_requirements": min_reqs,
                    "checks": checks, "all_ok": all_ok,
                    "qualified_months": months, "bonus": bonus,
                    "total": base_pay + bonus,
                    "claimed": is_current && claimed.contains(id),
                    "status": status,
                })
            }
        },
    ))
    .await;
    Ok(ok(serde_json::json!({
        "period": period,
        "metrics": metrics,
        // 月末自动结算 + 补领窗口提示（前端展示用）
        "claim_window_days": claim_window,
        "assigned": items,
    })))
}

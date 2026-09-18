//! M21 绩效考核 + M22 任务中心 + M19 保种区前台 HTTP 接口。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_ops(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M21 绩效考核
        .service(jixiao_types)
        .service(jixiao_me)
        .service(jixiao_claim)
        .service(jixiao_my)
        // M22 任务中心
        .service(task_list)
        .service(task_overview)
        .service(task_claim)
        // 考核引擎（0093）：我的考核进度（含当前值 vs 目标）
        .service(my_exams)
        // M19 保种区
        .service(preserve_list)
        .service(preserve_claim)
        // 复活任务（0073，U3D Graveyard 口径）
        .service(resurrection_list)
        .service(resurrection_claim)
        .service(resurrection_my)
        // M28 插件
        .service(plugins_overview)
}

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
    "uploaded",           // 上传增量（字节，当月 traffic_ledger 聚合）
    "downloaded",         // 下载增量（字节）
    "uploads",            // 发种增量（当月新发布数）
    "seeding_count",      // 当前做种种子数（现值口径）
    "seed_size",          // 当前做种体积（字节，现值口径）
    "seed_size_tb",       // 当前做种体积（TB，现值口径）
    "seed_hours",         // 当月做种时长（小时，基线差值）
    "avg_seed_hours",     // 平均做种时长（小时/个：时长差值 ÷ 期内活跃种子数，NP 平均做种口径）
    "seed_days",          // 当月有做种活动的天数（近似：snatches.last_seen_at 按天去重）
    "seed_points_delta",  // 做种积分增量（1 积分 = 1 小时做种，与 exam 引擎 task_jobs 同源口径）
    "spark_delta",        // 火花增量（当月 spark_ledger 正向流水合计）
    "ops",                // 当月操作数（audit_log 按月）
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
async fn jixiao_types(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, JixiaoTypeRow>(
        "SELECT id, name, base_pay, metrics, min_requirements, description FROM jixiao_types ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 读取整数型站点设置（缺省取 dft）
async fn jixiao_setting_i64(db: &sqlx::PgPool, name: &str, dft: i64) -> i64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
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
    let (step, pct) = match rules
        .as_ref()
        .and_then(|r| r.as_object())
        .map(|r| {
            (
                r.get("months_per_step").and_then(|v| v.as_i64()),
                r.get("percent_per_step").and_then(|v| v.as_i64()),
            )
        }) {
        Some((Some(step), Some(pct))) if step > 0 && pct >= 0 => (step, pct),
        _ => (
            jixiao_setting_i64(db, "jixiao_bonus_months_per_step", 3).await.max(1),
            jixiao_setting_i64(db, "jixiao_bonus_percent_per_step", 10).await,
        ),
    };
    base_pay * pct / 100 * (qualified_months / step)
}

/// 我的绩效考核总览（NP jixiao.php 口径）：分配给我的岗位 + 本期实时指标 vs 要求 + 达标状态。
/// 未被分配岗位时 assigned 为空数组（前端展示引导文案，而不是让用户对着全岗位表点领取吃报错）。
#[get("/jixiao/me")]
async fn jixiao_me(
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
        now_site.format("%d").to_string().parse::<i64>().unwrap_or(99)
    };
    let window = jixiao_setting_i64(&state.repo.db, "jixiao_claim_window_days", 7).await;
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
        compute_metrics(&state.repo.db, auth.id, prev_period.as_deref().unwrap()).await?
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
    let items: Vec<serde_json::Value> = futures_util::future::join_all(
        assigned.iter().map(|(id, name, row_period, base_pay, _m, min_reqs, status, settled_months)| {
            let metrics = if row_period == &period { metrics.clone() } else { prev_metrics.clone() };
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
        }),
    )
    .await;
    Ok(ok(serde_json::json!({
        "period": period,
        "metrics": metrics,
        // 月末自动结算 + 补领窗口提示（前端展示用）
        "claim_window_days": claim_window,
        "assigned": items,
    })))
}

/// 指标采集：全部来自系统流水（announce 统计/保种表/操作日志），零手工填报（M21 验收）。
///
/// 0106 口径：
///   做种时长类用「月度基线快照差值」（jixiao_baseline_snapshots，主口径；
///   快照缺失时用 jixiao_claims 行级 base_* 兜底——admin 分配时记的当刻累计值）
///   现值类（seeding_count/seed_size/seed_size_tb）不参与差值
///   ops 修复为按月过滤（旧实现是全历史计数，口径不符）
pub async fn compute_metrics(
    db: &sqlx::PgPool,
    user_id: i64,
    period: &str, // "2026-09"
) -> DomainResult<serde_json::Value> {
    // ── 期初基线：月度快照优先，缺则行级 base_*（admin 分配时记），再缺则 0 ──
    // 只需做种时长基线（seed_hours 差值）；流量/发布数直接按 traffic_ledger/torrents
    // 的当月聚合，不依赖基线。
    let prev = jixiao_prev_period(period);
    let mut base_seed: Option<i64> = None;
    if let Some(p) = &prev {
        base_seed = sqlx::query_scalar(
            "SELECT seed_seconds FROM jixiao_baseline_snapshots \
             WHERE user_id = $1 AND period = $2",
        )
        .bind(user_id)
        .bind(p)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    }
    let base_seed: i64 = match base_seed {
        Some(v) => v,
        None => sqlx::query_scalar(
            "SELECT base_seed_seconds FROM jixiao_claims \
             WHERE user_id = $1 AND period = $2 AND metrics_snapshot->>'source' = 'admin' \
             ORDER BY id LIMIT 1",
        )
        .bind(user_id)
        .bind(period)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten()
        .unwrap_or(0),
    };

    // ── 当月流量（traffic_ledger 按小时窗，月度聚合；基线差值口径对齐） ──
    let (up_month, down_month): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint \
         FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or((0, 0));

    // ── 做种时长：现值 - 期初基线（snatches.seeded_seconds 是 int，显式 ::bigint） ──
    let seed_seconds_now: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(seeded_seconds), 0)::bigint FROM snatches WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_hours = ((seed_seconds_now - base_seed).max(0)) / 3600;

    // ── 当月做种活动天数（近似）：snatches.last_seen_at 按天去重 ──
    let seed_days: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT date_trunc('day', s.last_seen_at)) FROM snatches s \
         WHERE s.user_id = $1 AND to_char(s.last_seen_at, 'YYYY-MM') = $2 AND s.seeding",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let seeding_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id = $1 AND seeding",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size),0)::bigint FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 AND s.seeding",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let uploads: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    // ops 修复：按月过滤（旧实现全历史计数，与「当月操作数」考核口径不符）
    let ops: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // 平均做种时长（NP Exam 口径）：时长差值 ÷ 期内有做种活动的种子数（无则 1）
    let active_seed_torrents: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT torrent_id) FROM snatches \
         WHERE user_id = $1 AND to_char(last_seen_at, 'YYYY-MM') = $2 AND seeding",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let avg_seed_hours = seed_hours / active_seed_torrents.max(1);

    // 火花增量（当月正向流水合计）与做种积分增量（1 积分 = 1 小时，与 exam 引擎同源）
    let spark_delta: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount),0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND amount > 0 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "uploaded": up_month, "downloaded": down_month, "uploads": uploads,
        "seeding_count": seeding_count, "seed_size": seed_size,
        "seed_size_tb": seed_size / 1_099_511_627_776, // 1 TB = 2^40
        "seed_hours": seed_hours, "seed_days": seed_days, "ops": ops,
        "avg_seed_hours": avg_seed_hours,
        "seed_points_delta": seed_hours, // 1 积分 = 1 小时做种（task_jobs.rs P1-6 口径）
        "spark_delta": spark_delta,
    }))
}

#[derive(Deserialize)]
struct ClaimReq {
    type_id: i64,
    /// YYYY-MM；缺省当月。允许上一期（月末结算后补领窗口内）
    #[serde(default)]
    period: Option<String>,
}

/// 站点时区（UTC+8）下的今天与 period 月末之间的天数差（正数 = period 已结束 N 天）
fn days_after_period_end(period: &str) -> i64 {
    let (y, m): (i32, u32) = {
        let mut it = period.split('-');
        let y = it.next().and_then(|v| v.parse().ok()).unwrap_or(2026);
        let m = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
        (y, m)
    };
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    use chrono::TimeZone;
    let month_end = chrono::FixedOffset::east_opt(8 * 3600)
        .and_then(|tz| tz.with_ymd_and_hms(ny, nm, 1, 0, 0, 0).single())
        .unwrap_or_else(|| chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap()));
    let now_cst = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
    (now_cst - month_end).num_days()
}

#[post("/jixiao/claim")]
async fn jixiao_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let now_period = chrono::Utc::now().format("%Y-%m").to_string();
    let period = body.period.clone().unwrap_or(now_period.clone());

    // 补领窗口：只能领当月或上月（且上月必须在窗口期内；月末已自动结算过的不可再领）
    if period != now_period {
        if Some(period.clone()) != jixiao_prev_period(&now_period) {
            return Err(DomainError::Validation("只能领取本期或上一期的绩效工资".into()));
        }
        let window = jixiao_setting_i64(&state.repo.db, "jixiao_claim_window_days", 7).await;
        let elapsed = days_after_period_end(&period);
        if elapsed < 0 {
            return Err(DomainError::Validation("该期尚未结束".into()));
        }
        if elapsed > window {
            return Err(DomainError::Validation(format!(
                "补领窗口（{window} 天）已过"
            )));
        }
    }

    // 审计修复（P0 无上限铸币）：考核工资必须由 admin 经 /admin/users/{id}/jixiao
    // 分配岗位后才能领取。旧逻辑任何登录用户可自选任意 type_id 领取（min_requirements
    // 全站为空使校验空转），每用户每月可扫全部岗位 ≈89,500 火花。
    let assigned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims \
         WHERE user_id = $1 AND type_id = $2 AND period = $3 \
           AND metrics_snapshot->>'source' = 'admin')",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !assigned {
        return Err(DomainError::Validation(
            "该期管理组尚未为你分配此考核岗位，无法领取".into(),
        ));
    }

    // 幂等：本期已结算（登记行已推进到 status=1）不可再领；
    // worker 月末自动结算与本人补领是**同一行**的 CAS 互斥（0106 单行设计）
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims \
         WHERE user_id = $1 AND type_id = $2 AND period = $3 \
           AND metrics_snapshot->>'source' = 'admin' AND settled_at IS NOT NULL)",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if claimed {
        return Err(DomainError::Validation("本期工资已发放（自动结算或本人领取）".into()));
    }

    let t: Option<(String, i64, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT name, base_pay, metrics, min_requirements FROM jixiao_types WHERE id = $1",
    )
    .bind(body.type_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, base_pay, _metrics, min_reqs)) = t else {
        return Err(DomainError::NotFound(body.type_id));
    };

    let metrics = compute_metrics(&state.repo.db, auth.id, &period).await?;
    // 最低要求线校验（低于要求不发基本工资 —— 旧站口径）
    for (key, min) in min_reqs.as_object().unwrap_or(&serde_json::Map::new()) {
        let actual = metrics.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let required = min.as_i64().unwrap_or(0);
        if required > 0 && actual < required {
            return Err(DomainError::Validation(format!(
                "「{name}」考核未达标：{key} 需 {required}，实际 {actual}"
            )));
        }
    }
    // 达标月数加成：每累计 N 个达标月 +P%（配置化，旧实现硬编码 /10*(m/3)）
    // 达标月数（**含本期**：历史 status=1 行数 + 1，本期达标正在领取）。
    // worker 结算侧同口径（months_before + 1）——首月即计入加成档位。
    let qualified_months: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND status = 1",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0)
        + 1;
    let bonus = jixiao_bonus(&state.repo.db, body.type_id, base_pay, qualified_months).await;
    let total = base_pay + bonus;

    // 单行设计（0106 修正）：登记与发放是同一行的状态流转——
    // UNIQUE(user_id, type_id, period) 下不可能同时存在 admin 登记行和用户领取行，
    // 旧实现「登记行 + 再 INSERT 领取行」必然撞唯一约束（表现为永远「本期已领取过工资」）。
    // 领取 = 把登记行 CAS 推进到 status=1（与 worker 结算同一互斥口径）。
    let claim_id: i64 = sqlx::query_scalar(
        "UPDATE jixiao_claims SET status = 1, settled_at = now(), amount = $3, bonus_paid = $4, \
                metrics_at_settle = $5, \
                metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"self\"}'::jsonb \
         WHERE user_id = $1 AND type_id = $2 AND period = $6 \
           AND metrics_snapshot->>'source' = 'admin' AND settled_at IS NULL AND status = 0 \
         RETURNING id",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(total)
    .bind(bonus)
    .bind(metrics.clone())
    .bind(&period)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or_else(|| DomainError::Validation("本期已领取过工资或已由系统结算".into()))?;

    let idem = format!("jixiao:{}:{}", claim_id, period);
    // kind 修正：旧实现误复用 task_reward（报表与任务奖励混同）
    earn_spark(&state.repo.db, auth.id, total, "jixiao_reward", &idem).await?;
    state
        .repo
        .audit(Some(auth.id), "jixiao_claim", Some(claim_id))
        .await;
    Ok(ok(
        serde_json::json!({ "claim_id": claim_id, "type": name, "base": base_pay, "bonus": bonus, "total": total }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ClaimRow {
    id: i64,
    type_name: String,
    period: String,
    amount: i64,
    #[serde(default)]
    bonus_paid: i64,
    #[serde(default)]
    status: i16,
    /// 发放来源：worker=月末自动结算 / self=本人领取（展示「自动发放/手动领取」）
    #[sqlx(default)]
    source: Option<String>,
    claimed_at: chrono::DateTime<chrono::Utc>,
}

#[get("/jixiao/my")]
async fn jixiao_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.id, t.name AS type_name, c.period, c.amount, c.bonus_paid, c.status, \
                c.metrics_snapshot->>'settle_by' AS source, c.claimed_at \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.user_id = $1 AND c.status = 1 \
         ORDER BY c.settled_at DESC NULLS LAST, c.id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M22 任务中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TaskRow {
    id: i64,
    name: String,
    metric: serde_json::Value,
    reward: i64,
    penalty: i64,
    claim_limit: Option<i32>,
    claimed: i64,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    /// 五档任务卡（包子站 task.php 口径）
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[sqlx(default)]
    fee: i64,
    #[sqlx(default)]
    duration_days: i32,
    /// 0094 废弃：名额统一走 claim_limit；仅为 task-board 兼容下发（值 = claim_limit）
    #[sqlx(default)]
    quota_total: i32,
    #[sqlx(default)]
    sort: i32,
    /// 当前用户是否已领取
    #[sqlx(default)]
    claimed_by_me: bool,
}

#[get("/tasks")]
async fn task_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let uid = require_auth(&req, &state).await.ok().map(|a| a.id);
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT t.id, t.name, t.metric, t.reward, t.penalty, t.claim_limit, \
            (SELECT count(*) FROM task_claims tc WHERE tc.task_id = t.id)::bigint AS claimed, \
            t.starts_at, t.ends_at, t.tier, t.subtitle, t.fee, t.duration_days, \
            COALESCE(t.claim_limit, t.quota_total) AS quota_total, t.sort, \
            EXISTS(SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = $1) AS claimed_by_me \
         FROM tasks t WHERE now() BETWEEN t.starts_at AND t.ends_at \
         ORDER BY t.sort NULLS LAST, t.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 任务系统总览（包子站 task.php 区块口径）：商店/动态/统计/我的记录
#[get("/tasks/overview")]
async fn task_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;

    // 商店
    let shop: Vec<(String, String, String, i32, f64, i32)> = sqlx::query_as(
        "SELECT name, span, require_tier, require_count, cost::float8, stock \
         FROM task_shop_items ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let shop_json: Vec<serde_json::Value> = shop
        .iter()
        .map(|(n, s, t, c, cost, stock)| {
            serde_json::json!({
                "name": n, "span": s, "require_tier": t, "require_count": c,
                "cost": cost, "stock": stock,
            })
        })
        .collect();

    // 最新动态（领取流）
    let feed: Vec<(String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT u.username, t.name, now() FROM task_claims c \
         JOIN users u ON u.id = c.user_id JOIN tasks t ON t.id = c.task_id \
         ORDER BY c.id DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let feed_json: Vec<serde_json::Value> = feed
        .iter()
        .map(|(u, name, ts)| serde_json::json!({ "user": u, "task": name, "at": ts.to_rfc3339() }))
        .collect();

    // 统计：进行中/已完成/失败 + 各档完成率
    let (ongoing, done, failed): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 0), count(*) FILTER (WHERE status = 1), \
                count(*) FILTER (WHERE status = 2) FROM task_claims",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let tier_stats: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT t.tier, count(*) AS total, count(*) FILTER (WHERE c.status = 1) AS done \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         GROUP BY t.tier ORDER BY t.tier NULLS LAST",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let tiers_json: Vec<serde_json::Value> = tier_stats
        .iter()
        .map(|(tier, total, done)| {
            let pct = if *total == 0 {
                0.0
            } else {
                (*done as f64 / *total as f64) * 100.0
            };
            serde_json::json!({ "tier": tier, "total": total, "done": done, "pct": pct })
        })
        .collect();

    // 我的任务记录：含进行中任务的实时进度（current 与 /me/exams、task_settle 同口径）
    let mine: Vec<(
        i64,
        String,
        i16,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        i64,
        Option<String>,
        i64,
        i64,
        i64,
        serde_json::Value,
    )> = sqlx::query_as(
        "SELECT t.id, t.name, c.status, c.claimed_at, c.settled_at, \
                (c.claimed_at + (t.duration_days || ' days')::interval) AS deadline, \
                t.reward, t.tier, c.base_uploaded, c.base_seed_seconds, c.base_uploads, t.metric \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 ORDER BY c.id DESC LIMIT 20",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 指标现值一次性取齐（四源与 task_settle/my_exams 一致）
    let stats: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0), \
                (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = u.id) \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (cur_up, cur_seed, cur_uploads, cur_subtitles) = stats.unwrap_or((0, 0, 0, 0));

    let mine_json: Vec<serde_json::Value> = mine
        .iter()
        .map(|(id, name, status, at, settled, deadline, reward, tier, b_up, b_seed, b_uploads, metric)| {
            let (up, seed, ups) = if tier.is_some() {
                (cur_up, cur_seed, cur_uploads)
            } else {
                (cur_up - b_up, cur_seed - b_seed, cur_uploads - b_uploads)
            };
            serde_json::json!({
                "task_id": id, "name": name, "status": status,
                "claimed_at": at.to_rfc3339(), "settled_at": settled.map(|s| s.to_rfc3339()),
                "deadline": deadline.map(|d| d.to_rfc3339()),
                "reward": reward,
                "metric": metric,
                "current": { "uploaded": up, "seed_seconds": seed, "uploads": ups, "subtitles": cur_subtitles },
            })
        })
        .collect();

    Ok(ok(serde_json::json!({
        "shop": shop_json,
        "feed": feed_json,
        "stats": { "ongoing": ongoing, "done": done, "failed": failed, "tiers": tiers_json },
        "my_records": mine_json,
    })))
}

#[derive(Deserialize)]
struct TaskClaimReq {
    task_id: i64,
}

#[post("/tasks/claim")]
async fn task_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TaskClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;

    // 一次性取任务配置：存在性/时段/限领/报名费/等级门槛/指标
    let task: Option<(bool, Option<i32>, i64, i32, serde_json::Value)> = sqlx::query_as(
        "SELECT now() BETWEEN starts_at AND ends_at, claim_limit, fee, target_class, metric \
         FROM tasks WHERE id = $1",
    )
    .bind(body.task_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((in_window, claim_limit, fee, target_class, metric_json)) = task else {
        return Err(DomainError::NotFound(body.task_id));
    };
    if !in_window {
        return Err(DomainError::Validation("任务不在可领取时段内".into()));
    }
    // claim_limit 语义：NULL = 不限领（此前 NULL 误判 404）；0 = 已停止领取；>0 = 名额上限
    if claim_limit == Some(0) {
        return Err(DomainError::Validation("该任务已停止领取".into()));
    }
    // 目标等级门槛（此前缺失校验：低等级可领高等级任务）
    let user_class: Option<i32> = sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(user_class) = user_class else {
        return Err(DomainError::NotFound(auth.id));
    };
    if user_class < target_class {
        return Err(DomainError::Validation("等级未达到该任务的领取门槛".into()));
    }
    // 指标有效性：metric 至少含一个已知键；否则认领会悬挂到超时被判失败并扣罚金
    const KNOWN_METRIC_KEYS: [&str; 6] = [
        "upload_delta",
        "download_delta",
        "seed_points_delta",
        "seed_seconds_delta",
        "uploads",
        "subtitles",
    ];
    let has_target = metric_json
        .as_object()
        .is_some_and(|o| KNOWN_METRIC_KEYS.iter().any(|k| o.contains_key(*k)));
    if !has_target {
        return Err(DomainError::Validation(
            "任务指标配置无效，暂不可领取".into(),
        ));
    }

    // 原子占位：名额校验与插入同语句（此前 count+INSERT 两步存在并发超领窗口）
    let limit_param: i64 = match claim_limit {
        Some(l) if l > 0 => i64::from(l),
        _ => i64::from(i32::MAX), // NULL = 不限领
    };
    let claim_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT $1, $2,
               u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1)
        FROM users u
        WHERE u.id = $2
          AND (SELECT count(*) FROM task_claims tc WHERE tc.task_id = $1) < $3
        RETURNING id
        "#,
    )
    .bind(body.task_id)
    .bind(auth.id)
    .bind(limit_param)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("已认领过该任务".into())
        }
        // 占位未插入只可能是名额满（用户存在性与等级已在前置校验确认）
        sqlx::Error::RowNotFound => DomainError::Validation("认领名额已满".into()),
        other => DomainError::Internal(other.into()),
    })?;

    // 报名费：先占位后扣费，扣费失败回滚占位（此前扣费在前，INSERT 失败会白扣费；
    // 幂等键 task_fee:{uid}:{task_id} 保证重试不会双扣）
    if fee > 0 {
        let idem = format!("task_fee:{}:{}", auth.id, body.task_id);
        if let Err(e) = crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            fee,
            "task_fee",
            &idem,
            "task",
            body.task_id,
        )
        .await
        {
            let _ = sqlx::query("DELETE FROM task_claims WHERE id = $1 AND user_id = $2")
                .bind(claim_id)
                .bind(auth.id)
                .execute(&state.repo.db)
                .await;
            return Err(e);
        }
    }
    Ok(ok(serde_json::json!({ "claim_id": claim_id })))
}

// ============ 考核引擎（0093，tasks 单表方案） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MyExamRow {
    task_id: i64,
    name: String,
    subtitle: Option<String>,
    metric: serde_json::Value,
    kind: String,
    period: String,
    status: i16,
    claimed_at: chrono::DateTime<chrono::Utc>,
    settled_at: Option<chrono::DateTime<chrono::Utc>>,
    deadline: Option<chrono::DateTime<chrono::Utc>>,
    reward: i64,
    penalty: i64,
    /// 基线快照（领取/派发时）：与现值相减得增量
    base_uploaded: i64,
    base_seed_seconds: i64,
    base_uploads: i64,
    tier: Option<String>,
}

/// 我的考核进度：task_claims 关联 kind IN ('onboard','periodic') 的任务，
/// current 按 task_settle 同款口径实时计算（tier 非空 = 累计口径按现值）。
#[get("/me/exams")]
async fn my_exams(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyExamRow> = sqlx::query_as(
        "SELECT t.id AS task_id, t.name, t.subtitle, t.metric, t.kind, t.period, \
                c.status, c.claimed_at, c.settled_at, \
                (c.claimed_at + (t.duration_days || ' days')::interval) AS deadline, \
                t.reward, t.penalty, \
                c.base_uploaded, c.base_seed_seconds, c.base_uploads, t.tier \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 AND t.kind IN ('onboard','periodic') \
         ORDER BY c.claimed_at DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 指标现值一次性取齐（与 task_settle 相同的四个数据源）
    let stats: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0), \
                (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = u.id) \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let current = stats.map(|(uploaded, seed_secs, uploads, subtitles)| {
                // 累计口径（tier 任务）：基线视为 0，直接报现值；否则报增量
                let (up, seed, ups) = if r.tier.is_some() {
                    (uploaded, seed_secs, uploads)
                } else {
                    (
                        uploaded - r.base_uploaded,
                        seed_secs - r.base_seed_seconds,
                        uploads - r.base_uploads,
                    )
                };
                serde_json::json!({
                    "uploaded": up,
                    "seed_seconds": seed,
                    "uploads": ups,
                    "subtitles": subtitles,
                })
            });
            serde_json::json!({
                "task_id": r.task_id,
                "name": r.name,
                "subtitle": r.subtitle,
                "metric": r.metric,
                "kind": r.kind,
                "period": r.period,
                "status": r.status,
                "claimed_at": r.claimed_at,
                "settled_at": r.settled_at,
                "deadline": r.deadline,
                "reward": r.reward,
                "penalty": r.penalty,
                "current": current.unwrap_or(serde_json::json!({})),
            })
        })
        .collect();
    Ok(ok(out))
}

// ============ M19 保种区 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PreserveRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    claimed_by: Option<String>,
    // 资源库行同构字段（保种区列表复用 TorrentTr 渲染，参考站口径）
    small_descr: Option<String>,
    category_id: i32,
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    leechers: i32,
    times_completed: i32,
    comments: i64,
    #[serde(rename = "official")]
    official_tag: bool,
    sticky: bool,
    anonymous: bool,
    promotion: Option<String>,
    promotion_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    poster: Option<String>,
    owner_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct PreserveQuery {
    /// scope: all/official/general；status: current/active/grace/expired
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    category: Option<i32>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    page: Option<i64>,
}

/// 保种区列表（包子站 requireseed.php 口径）：统计六格 + 筛选 + 分页 + 种子九列表格
#[get("/preserve")]
async fn preserve_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PreserveQuery>,
) -> DomainResult<impl Responder> {
    let page = q.page.unwrap_or(0).max(0);
    let per = 50i64;
    let kw = q
        .keyword
        .as_deref()
        .map(|k| crate::http::like_pattern(&k))
        .unwrap_or_else(|| "%".into());
    // 审计修复（P1）：scope/status 筛选此前声明即弃（#[allow(dead_code)]），前端表单
    // 提交被静默忽略。接线：scope=official/general 过滤官种位；status 按认领/延续
    // 状态过滤（current=全部在保、active=已被认领延续、grace=移出宽限中、expired=已移出）。
    let scope_ok = matches!(q.scope.as_deref(), Some("official") | Some("general"));
    let official_only = q.scope.as_deref() == Some("official");
    let general_only = q.scope.as_deref() == Some("general");
    // status=expired 需要查已移出行——主查询固定 exited_at IS NULL，expired 单独走分支
    if q.status.as_deref() == Some("expired") {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT sp.torrent_id, t.name, t.size FROM seed_preserve sp \
             JOIN torrents t ON t.id = sp.torrent_id \
             WHERE sp.exited_at IS NOT NULL \
               AND (NOT $1::bool OR t.official_tag) \
               AND (NOT $2::bool OR NOT t.official_tag) \
             ORDER BY sp.exited_at DESC LIMIT 50",
        )
        .bind(official_only)
        .bind(general_only)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(ok(rows));
    }
    let _ = scope_ok;

    let rows = sqlx::query_as::<_, PreserveRow>(
        "SELECT sp.torrent_id, t.name, t.size, t.seeders, u.username AS claimed_by, \
         t.small_descr, t.category_id, t.medium_id, t.grade_id, t.edition_id, \
         t.leechers, t.times_completed, \
         (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments, \
         t.official_tag, t.sticky, t.anonymous, \
         (SELECT p.kind::text FROM promotions p WHERE p.starts_at <= now() AND p.ends_at > now() \
            AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id)))) \
            ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion, \
         (SELECT p.ends_at FROM promotions p WHERE p.starts_at <= now() AND p.ends_at > now() \
            AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id)))) \
            ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at, \
         t.media_info->>'poster' AS poster, \
         CASE WHEN t.anonymous THEN NULL ELSE o.username END AS owner_name, \
         t.created_at \
         FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         LEFT JOIN users u ON u.id = sp.claimed_by \
         LEFT JOIN users o ON o.id = t.owner_id \
         WHERE sp.exited_at IS NULL AND t.approval_status = 1 \
           AND (NOT $5::bool OR t.official_tag) \
           AND (NOT $6::bool OR NOT t.official_tag) \
           AND (NOT $7::bool OR sp.claimed_by IS NOT NULL) \
           AND ($1::int IS NULL OR t.category_id = $1) \
           AND t.name ILIKE $2 \
         ORDER BY t.seeders ASC, sp.torrent_id LIMIT $3 OFFSET $4",
    )
    .bind(q.category)
    .bind(kw)
    .bind(per)
    .bind(page * per)
    .bind(official_only)
    .bind(general_only)
    .bind(q.status.as_deref() == Some("active"))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 统计六格（包子站保种区口径：保种中/延续中/官方保种/普通保种/今日新增/今日移出）
    let (preserving, _exited, total, today_in, today_out): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NOT NULL), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL), \
                (SELECT count(*) FROM seed_preserve WHERE claimed_at > now() - interval '1 day'), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at > now() - interval '1 day')",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let official = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM seed_preserve sp JOIN torrents t ON t.id = sp.torrent_id \
         WHERE sp.exited_at IS NULL AND t.official_tag",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 延续中 = 已被认领且仍在保种区（此前硬编码 0 永远显示 0）
    let continued: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL AND claimed_by IS NOT NULL",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);

    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "stats": {
            "preserving": preserving, "continued": continued,
            "official": official, "general": (preserving - official).max(0),
            "today_in": today_in, "today_out": today_out,
        },
        "page": page, "per_page": per,
    })))
}

#[derive(Deserialize)]
struct PreserveClaimReq {
    torrent_id: i64,
}

#[post("/preserve/claim")]
async fn preserve_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PreserveClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 认领即记录做种时长/上传量基线（NP claims.seed_time_begin/uploaded_begin 口径）
    let updated = sqlx::query(
        "UPDATE seed_preserve sp SET \
            claimed_by = $2, claimed_at = now(), last_settle_at = now(), \
            seed_time_begin = COALESCE(( \
                SELECT s.seeded_seconds FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0), \
            uploaded_begin = COALESCE(( \
                SELECT s.uploaded FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0) \
         WHERE sp.torrent_id = $1 AND sp.claimed_by IS NULL",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation("该种子已被认领或不在保种区".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "preserve_claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.torrent_id })))
}

// ============ M28 插件 ============

/// 插件清单（staff 可见）
#[get("/admin/plugins")]
async fn plugins_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PLUGINS_MANAGE).await?;
    Ok(ok(serde_json::json!({
        "plugins": state.plugins.list(),
        "hooks": ["on_user_login", "on_torrent_upload", "on_seeding_milestone"],
    })))
}

// ============ 复活任务（0073，U3D Graveyard 口径） ============

/// 可领取的死种列表：已过审、无做种、30 天无 snatches 活动且无未完成任务的种子。
/// 教育站定位：老教材种被救活的长期价值高，激励优先于清理。
#[get("/resurrections")]
async fn resurrection_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?; // 登录可见（未过审不可见由 approval_status 保证）
                                                   // 审计修复（P2）：旧版 ORDER BY t.created_at ASC 恒取最老的 50 条——新死种永远
                                                   // 排不进列表（30 天活动排除口径下“最老”几乎全是无人问津的陈种）。改为按
                                                   // “最近一次有人活跃的时间”倒序：新鲜死种（刚断种、还有救的价值）优先露出。
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT t.id, t.name, t.size FROM torrents t \
         WHERE t.approval_status = 1 AND t.seeders = 0 \
           AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.torrent_id = t.id \
                           AND s.last_seen_at > now() - interval '30 days') \
           AND NOT EXISTS (SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id AND r.status = 'open') \
         ORDER BY COALESCE((SELECT max(s.last_seen_at) FROM snatches s WHERE s.torrent_id = t.id), \
                           t.created_at) DESC, t.id DESC \
         LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ResClaimReq {
    torrent_id: i64,
}

/// 领取复活任务（一种一任务，CAS 防并发双领）；required_hours 领取时快照（可配）。
#[post("/resurrections/claim")]
async fn resurrection_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 领取门槛：不能救自己的种（U3D 口径——自己的死种自己救没有增量价值）
    let owner: i64 =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1")
            .bind(body.torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or(DomainError::NotFound(body.torrent_id))?;
    if owner == auth.id {
        return Err(DomainError::Validation("不能领取自己发布的种子".into()));
    }
    // value::int 得到的是 INT4，必须读 i32。原为 i64：解码期报 mismatched types，
    // 又被随后的 unwrap_or(240) 静默吞掉 —— 表现为 resurrection_hours 配置永远不生效、恒为 240。
    let hours: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'resurrection_hours'), 240)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(240);
    let inserted = sqlx::query(
        "INSERT INTO resurrections (torrent_id, user_id, required_hours, reward_sparks) \
         VALUES ($1, $2, $3, 5000) ON CONFLICT (torrent_id) DO NOTHING",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(hours)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if inserted == 0 {
        return Err(DomainError::Validation("该种子已有进行中的复活任务".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "resurrection.claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id,
        "required_hours": hours,
        "reward": "5000 火花 + 1 枚免费券 + 该种 7 天免费",
        "note": "worker 每小时自动验收：累计做种达标且当前仍在做种即发奖"
    })))
}

/// 我的复活任务
#[get("/resurrections/mine")]
async fn resurrection_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, i64, i32, chrono::DateTime<chrono::Utc>, String, i32)> = sqlx::query_as(
        "SELECT r.torrent_id, r.reward_sparks, r.required_hours, r.claimed_at, r.status, \
                COALESCE(s.seeded_seconds, 0) \
         FROM resurrections r \
         LEFT JOIN snatches s ON s.user_id = r.user_id AND s.torrent_id = r.torrent_id \
         WHERE r.user_id = $1 ORDER BY r.claimed_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

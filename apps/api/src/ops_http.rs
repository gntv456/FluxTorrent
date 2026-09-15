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

#[derive(sqlx::FromRow, serde::Serialize)]
struct JixiaoTypeRow {
    id: i64,
    name: String,
    base_pay: i64,
    metrics: serde_json::Value,
    min_requirements: serde_json::Value,
}

#[get("/jixiao/types")]
async fn jixiao_types(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, JixiaoTypeRow>(
        "SELECT id, name, base_pay, metrics, min_requirements FROM jixiao_types ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 指标采集：全部来自系统流水（announce 统计/保种表/操作日志），零手工填报（M21 验收）
async fn compute_metrics(
    db: &sqlx::PgPool,
    user_id: i64,
    period: &str, // "2026-09"
) -> DomainResult<serde_json::Value> {
    let record: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT \
            COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint, \
            COALESCE((SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id = $1 AND seeding),0)::bigint, \
            COALESCE((SELECT count(*) FROM audit_log WHERE actor_id = $1),0)::bigint \
         FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up, down, seeding_count, ops) = record.unwrap_or((0, 0, 0, 0));
    let uploads: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
    )
    .bind(user_id)
    .bind(period)
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
    Ok(serde_json::json!({
        "uploaded": up, "downloaded": down, "uploads": uploads,
        "seeding_count": seeding_count, "seed_size": seed_size, "ops": ops,
    }))
}

#[derive(Deserialize)]
struct ClaimReq {
    type_id: i64,
    #[serde(default)]
    period: Option<String>,
}

#[post("/jixiao/claim")]
async fn jixiao_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let period = body
        .period
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());

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
            "本月管理组尚未为你分配该考核岗位，无法领取".into(),
        ));
    }

    // 幂等：本期已领（UNIQUE 约束兜底 + 先查友好报错）
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND period = $3 AND NOT (metrics_snapshot->>'source' = 'admin'))",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if claimed {
        return Err(DomainError::Validation("本期已领取过工资".into()));
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
    let m = &metrics;
    for (key, min) in min_reqs.as_object().unwrap_or(&serde_json::Map::new()) {
        let actual = m.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let required = min.as_i64().unwrap_or(0);
        if required > 0 && actual < required {
            return Err(DomainError::Validation(format!(
                "「{name}」考核未达标：{key} 需 {required}，实际 {actual}"
            )));
        }
    }
    // 达标月数加成：每累计 3 个达标月 +10%
    let qualified_months: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND NOT (metrics_snapshot->>'source' = 'admin')",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let bonus = base_pay / 10 * (qualified_months / 3);
    let total = base_pay + bonus;

    let claim_id: i64 = sqlx::query_scalar(
        "INSERT INTO jixiao_claims (user_id, type_id, period, amount, metrics_snapshot) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.type_id)
    .bind(&period)
    .bind(total)
    .bind(metrics.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("本期已领取过工资".into())
        }
        other => DomainError::Internal(other.into()),
    })?;

    let idem = format!("jixiao:{}:{}", claim_id, period);
    earn_spark(&state.repo.db, auth.id, total, "task_reward", &idem).await?;
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
    claimed_at: chrono::DateTime<chrono::Utc>,
}

#[get("/jixiao/my")]
async fn jixiao_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, ClaimRow>(
        "SELECT c.id, t.name AS type_name, c.period, c.amount, c.claimed_at \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.user_id = $1 AND NOT (c.metrics_snapshot->>'source' = 'admin') ORDER BY c.id DESC LIMIT 20",
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
    let user_class: Option<i32> =
        sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
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
        return Err(DomainError::Validation("任务指标配置无效，暂不可领取".into()));
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
#[allow(dead_code)] // 兼容前端传入、后端暂未消费的筛选参数
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
           AND ($1::int IS NULL OR t.category_id = $1) \
           AND t.name ILIKE $2 \
         ORDER BY t.seeders ASC, sp.torrent_id LIMIT $3 OFFSET $4",
    )
    .bind(q.category)
    .bind(kw)
    .bind(per)
    .bind(page * per)
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
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT t.id, t.name, t.size FROM torrents t \
         WHERE t.approval_status = 1 AND t.seeders = 0 \
           AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.torrent_id = t.id \
                           AND s.last_seen_at > now() - interval '30 days') \
           AND NOT EXISTS (SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id AND r.status = 'open') \
         ORDER BY t.created_at LIMIT 50",
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
    let hours: i64 = sqlx::query_scalar(
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

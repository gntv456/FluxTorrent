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
        // M19 保种区
        .service(preserve_list)
        .service(preserve_claim)
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

    // 幂等：本期已领（UNIQUE 约束兜底 + 先查友好报错）
    let claimed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND period = $3)",
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
        "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2",
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
         WHERE c.user_id = $1 ORDER BY c.id DESC LIMIT 20",
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
            t.quota_total, t.sort, \
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
        .map(|(u, name, ts)| {
            serde_json::json!({ "user": u, "task": name, "at": ts.to_rfc3339() })
        })
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

    // 我的任务记录
    let mine: Vec<(i64, String, i16, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT t.id, t.name, c.status, now(), c.settled_at \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 ORDER BY c.id DESC LIMIT 20",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mine_json: Vec<serde_json::Value> = mine
        .iter()
        .map(|(id, name, status, at, settled)| {
            serde_json::json!({
                "task_id": id, "name": name, "status": status,
                "claimed_at": at.to_rfc3339(), "settled_at": settled.map(|s| s.to_rfc3339()),
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
    // 任务需在有效期内
    let window: Option<bool> = sqlx::query_scalar(
        "SELECT now() BETWEEN starts_at AND ends_at FROM tasks WHERE id = $1",
    )
    .bind(body.task_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if window != Some(true) {
        return Err(DomainError::Validation("任务不在可领取时段内".into()));
    }
    // 认领人数限流（旧站 23/100 口径 → 数据库计数 + 唯一约束）
    let limit: Option<i32> = sqlx::query_scalar("SELECT claim_limit FROM tasks WHERE id = $1")
        .bind(body.task_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(limit) = limit else {
        return Err(DomainError::NotFound(body.task_id));
    };
    if limit > 0 {
        let claimed: i64 =
            sqlx::query_scalar("SELECT count(*) FROM task_claims WHERE task_id = $1")
                .bind(body.task_id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(0);
        if claimed >= limit as i64 {
            return Err(DomainError::Validation("认领名额已满".into()));
        }
    }
    // 报名费（任务配置了 fee 时先扣，凭据 ref 指向任务；重复领取被唯一约束挡住，不会双扣）
    let fee: i64 = sqlx::query_scalar("SELECT fee FROM tasks WHERE id = $1")
        .bind(body.task_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if fee > 0 {
        let idem = format!("task_fee:{}:{}", auth.id, body.task_id);
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            fee,
            "task_fee",
            &idem,
            "task",
            body.task_id,
        )
        .await?;
    }
    // 指标基线快照（base + delta 结算口径，对齐好学站 UserTaskRecord）
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT $1, $2,
               u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1)
        FROM users u WHERE u.id = $2
        RETURNING id
        "#,
    )
    .bind(body.task_id)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("已认领过该任务".into())
        }
        other => DomainError::Internal(other.into()),
    })?;
    Ok(ok(serde_json::json!({ "claim_id": id })))
}

// ============ M19 保种区 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PreserveRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    claimed_by: Option<String>,
    // 资源库行同构字段（保种区列表复用 TorrentTr 渲染，好学站口径）
    small_descr: Option<String>,
    category_id: i32,
    medium_id: i32,
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
                (SELECT count(*) FROM seed_preserve), \
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

    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "stats": {
            "preserving": preserving, "continued": 0,
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

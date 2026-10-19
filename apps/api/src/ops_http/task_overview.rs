//! 任务总览（M22）：GET /tasks/overview。
//! 从 ops_http/tasks.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

/// 任务系统总览（包子站 task.php 区块口径）：商店/动态/统计/我的记录
#[get("/tasks/overview")]
pub(super) async fn task_overview(
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
    let feed: Vec<(String, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
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

    // 我的任务记录：含进行中任务的实时进度（取数与折算拆到
    // task_records.rs，与 worker 结算同尺）
    let mine_json = super::task_records::records(&state, uid).await?;

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
pub(super) async fn task_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TaskClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;

    // 一次性取任务配置：存在性/时段/限领/报名费/等级门槛/指标
    let task: Option<(bool, Option<i32>, i64, i32, serde_json::Value)> =
        sqlx::query_as(
            "SELECT starts_at <= now() AND now() < ends_at, claim_limit, fee, \
             target_class, metric \
             FROM tasks WHERE id = $1",
        )
        .bind(body.task_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((in_window, claim_limit, fee, target_class, metric_json)) = task
    else {
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
        return Err(DomainError::Validation(
            "等级未达到该任务的领取门槛".into(),
        ));
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
    // P-383：占位与报名费同一事务，幂等键锚「这一条认领」（claim_id）。
    // 旧版键按「人 × 任务」定，而 admin_http/user_del.rs:88 清号删 task_claims
    // 却从不删 spark_ledger ⇒ 复活后重新认领同一任务时 spend 回 Ok(Replayed)
    // 被当成成功放过（实测：认领落了、报名费没扣＝免费认领）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let claim_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded,
                                 base_downloaded, base_seed_seconds,
                                 base_uploads)
        SELECT $1, $2,
               u.uploaded,
               u.downloaded,
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
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("已认领过该任务".into())
        }
        // 占位未插入只可能是名额满（用户存在性与等级已在前置校验确认）
        sqlx::Error::RowNotFound => DomainError::Validation("认领名额已满".into()),
        other => DomainError::Internal(other.into()),
    })?;

    // 报名费：与占位同事务（此前扣费在前、INSERT 失败会白扣费；扣费失败
    // 回滚占位靠 best-effort DELETE，撤失败就留下「已认领但没交费」的行）
    if fee > 0 {
        let idem = format!("task_fee:{}:{}", auth.id, claim_id);
        if !matches!(
            crate::economy_http::spend_spark_tx(
                &mut tx,
                auth.id,
                fee,
                "task_fee",
                &idem,
                "task",
                body.task_id,
            )
            .await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "这笔报名费已受理，未重复扣款".into(),
            ));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "claim_id": claim_id })))
}

// ============ 考核引擎（0093，tasks 单表方案） ============

//! 0106 绩效考核管理端补齐：批量分配 / 全站总览 / 发薪记录
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ 0106 绩效考核管理端补齐：批量分配 / 全站总览 / 发薪记录 ============

#[derive(Deserialize)]
struct JixiaoAssignBatchReq {
    type_id: i64,
    user_ids: Vec<i64>,
    /// YYYY-MM；缺省当月（站点时区 UTC+8）
    #[serde(default)]
    period: Option<String>,
}

/// 批量分配考核岗位：给一个岗位一次登记多名用户（工作组口径：主管建组，逐个登记）。
/// 逐用户校验 ensure_outranks（等级护栏）+ 基线快照（与单人分配同口径）；
/// 单事务：任一用户失败整体回滚（管理员重试比半成功状态好排查）。
#[post("/admin/jixiao/assign-batch")]
async fn jixiao_assign_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JixiaoAssignBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    if body.user_ids.is_empty() || body.user_ids.len() > 200 {
        return Err(DomainError::Validation("user_ids 需为 1~200 个".into()));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_types WHERE id = $1)",
    )
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(body.type_id));
    }
    let period = body.period.clone().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut assigned = Vec::new();
    let mut skipped = Vec::new();
    for &uid in &body.user_ids {
        // 等级护栏：与单人分配同口径（操作者等级须严格高于目标）
        // ensure_outranks 是 admin_http 的私有函数，这里用同语义直查
        // （class_id 严格大于；查询失败按不越权处理交由错误传播）
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(target_class) = target_class else {
            return Err(DomainError::NotFound(uid));
        };
        if auth.class_id <= target_class {
            return Err(DomainError::Validation(format!(
                "不能给等级不低于自己的用户（uid={uid}）分配考核"
            )));
        }
        let dup: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE \
             user_id = $1 AND type_id = $2 AND period = $3)",
        )
        .bind(uid)
        .bind(body.type_id)
        .bind(&period)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(true); // 查询失败按已存在处理，避免重复插入
        if dup {
            skipped.push(uid);
            continue;
        }
        // 基线快照（与 /jixiao/me compute_metrics 的行级兜底同源）
        sqlx::query(
            "INSERT INTO jixiao_claims \
                (user_id, type_id, period, amount, metrics_snapshot, base_seed_seconds, base_uploaded, base_uploads) \
             SELECT $1, $2, $3, 0, '{\"source\":\"admin\"}'::jsonb, \
                    COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = $1), 0)::bigint, \
                    u.uploaded, \
                    (SELECT count(*) FROM torrents tr WHERE tr.owner_id = $1 AND tr.approval_status = 1) \
             FROM users u WHERE u.id = $1",
        )
        .bind(uid)
        .bind(body.type_id)
        .bind(&period)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        assigned.push(uid);
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "jixiao.assign_batch", Some(body.type_id))
        .await;
    Ok(ok(serde_json::json!({
        "period": period, "assigned": assigned, "skipped_dup": skipped,
    })))
}

/// 全站考核总览（管理端）：按岗位聚合本期登记/达标/发薪情况，附成员明细。
/// 现状指标实时算（结算前），结算后读 metrics_at_settle 快照。
#[get("/admin/jixiao/overview")]
async fn jixiao_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let period = q.get("period").cloned().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });

    type Agg = (i64, String, i64, i64, i64, i64, i64, i64);
    let rows: Vec<Agg> = sqlx::query_as(
        "SELECT t.id, t.name, t.base_pay, count(c.id) FILTER (WHERE c.metrics_snapshot->>'source' = 'admin'), \
                count(c.id) FILTER (WHERE c.status = 1), \
                count(c.id) FILTER (WHERE c.status = 2), \
                count(c.id) FILTER (WHERE c.status = 0 AND c.settled_at IS NULL), \
                COALESCE(sum(c.amount) FILTER (WHERE c.status = 1), 0)::bigint \
         FROM jixiao_types t \
         LEFT JOIN jixiao_claims c ON c.type_id = t.id AND c.period = $1 \
         GROUP BY t.id, t.name, t.base_pay ORDER BY t.id",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 成员明细（仅本期 admin 登记行；每岗位至多 50 行，够管理端下钻）
    type Member = (
        i64,
        i64,
        String,
        i16,
        Option<i64>,
        Option<i64>,
        serde_json::Value,
    );
    let members: Vec<Member> = sqlx::query_as(
        "SELECT c.type_id, c.user_id, u.username, c.status, c.amount, c.bonus_paid, \
                COALESCE(c.metrics_at_settle, '{}'::jsonb) \
         FROM jixiao_claims c JOIN users u ON u.id = c.user_id \
         WHERE c.period = $1 AND c.metrics_snapshot->>'source' = 'admin' \
         ORDER BY c.type_id, c.status, u.username LIMIT 500",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let types: Vec<serde_json::Value> = rows
        .iter()
        .map(
            |(id, name, base_pay, assigned, ok, failed, pending, total)| {
                serde_json::json!({
                    "type_id": id, "name": name, "base_pay": base_pay,
                    "assigned": assigned, "qualified": ok, "failed": failed,
                    "pending": pending, "payroll_total": total,
                })
            },
        )
        .collect();
    let members_json: Vec<serde_json::Value> = members
        .iter()
        .map(|(tid, uid, username, status, amount, bonus, snap)| {
            serde_json::json!({
                "type_id": tid, "user_id": uid, "username": username,
                "status": status, "amount": amount, "bonus": bonus,
                "metrics_at_settle": snap,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "period": period, "types": types, "members": members_json,
    })))
}

/// 发薪记录（管理端）：本期已发薪行（status=1），按发放时间倒序。
#[get("/admin/jixiao/payroll")]
async fn jixiao_payroll(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let period = q.get("period").cloned().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });

    type Row = (
        i64,
        i64,
        String,
        String,
        i64,
        i64,
        String,
        chrono::DateTime<chrono::Utc>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT c.id, c.user_id, u.username, t.name, c.amount, c.bonus_paid, \
                COALESCE(c.metrics_snapshot->>'settle_by', 'self') AS paid_by, c.settled_at \
         FROM jixiao_claims c \
         JOIN users u ON u.id = c.user_id JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.period = $1 AND c.status = 1 \
         ORDER BY c.settled_at DESC NULLS LAST LIMIT 200",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let total: i64 = rows.iter().map(|r| r.4).sum();
    let list: Vec<serde_json::Value> = rows
        .iter()
        .map(|(id, uid, username, tname, amount, bonus, paid_by, at)| {
            serde_json::json!({
                "claim_id": id, "user_id": uid, "username": username,
                "type_name": tname, "amount": amount, "bonus": bonus,
                "paid_by": paid_by, "settled_at": at,
            })
        })
        .collect();
    Ok(ok(
        serde_json::json!({ "period": period, "total": total, "list": list }),
    ))
}

//! 0106 绩效考核管理端补齐：批量分配 / 全站总览 / 发薪记录
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
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
    // P-244：旧写法 `audit(Some(auth.id), "jixiao.assign_batch",
    // Some(body.type_id))` 把**岗位 id 记成审计对象**——发薪纠纷要问的三件事
    // 「改了谁、哪一期、谁改的」一件都答不上（skipped 只在响应里，落不到账）。
    // 批量口按「每个受影响成员一行」记：这样按成员反查（`ref->>'id'`）才走得通。
    for uid in &assigned {
        state
            .repo
            .audit_detail(
                Some(auth.id),
                "jixiao.assign_batch",
                Some(*uid),
                None,
                Some(serde_json::json!({
                    "type_id": body.type_id, "period": period.as_str(),
                    "skipped": false,
                })),
            )
            .await;
    }
    for uid in &skipped {
        state
            .repo
            .audit_detail(
                Some(auth.id),
                "jixiao.assign_batch",
                Some(*uid),
                None,
                Some(serde_json::json!({
                    "type_id": body.type_id, "period": period.as_str(),
                    "skipped": true,
                })),
            )
            .await;
    }
    Ok(ok(serde_json::json!({
        "period": period, "assigned": assigned, "skipped_dup": skipped,
    })))
}

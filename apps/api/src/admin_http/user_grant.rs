//! 用户详情页操作（勋章/道具/考核登记）。
//! 从 admin_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 详情页分配考核（参考站「分配考核」口径）：把用户登记为某考核岗位（jixiao_claims，当期）
#[derive(Deserialize)]
struct AssignJixiaoReq {
    type_id: i64,
    /// YYYY-MM；缺省当月
    #[serde(default)]
    period: Option<String>,
    #[serde(default)]
    amount: Option<i64>,
}

#[post("/admin/users/{id}/jixiao")]
async fn user_assign_jixiao(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AssignJixiaoReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-240：登记的是**待发金额**（settled_at IS NULL 即未发放），过去只判 staff()
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::JIXIAO_ASSIGN,
    )
    .await?;
    let uid = path.into_inner();
    // 等级护栏（审计修复）：90+ 可给 94/99 登记考核属越权，须严格高于目标
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    // P-242：旧写法 `.unwrap_or(false)` 把「查不到/查失败」混为一谈，
    // 这里改成错误传播；同时把该岗位的基准一并取出，用作金额上界
    // （数据推导，不拍脑袋定数字）。
    let base: Option<i64> = sqlx::query_scalar(
        "SELECT base_pay FROM jixiao_types WHERE id = $1",
    )
    .bind(body.type_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(base) = base else {
        return Err(DomainError::NotFound(body.type_id));
    };
    // P-240：登记的是待发金额——负数=凭空倒扣，无上限=手滑可写天量。
    let amount = body.amount.unwrap_or(0);
    if amount < 0 || amount > base * 3 {
        return Err(DomainError::Validation(format!(
            "金额需在 0-{} 之间（该岗位基准 {} 的 3 倍内）",
            base * 3, base
        )));
    }
    let period = body.period.clone().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 \
         AND type_id = $2 AND period = $3)",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // P-242：dup 查询失败旧写法当「没登记过」放行 ⇒ 同人同期同岗可
    // 登记两次 = 重复发钱
    if dup {
        return Err(DomainError::Validation(
            "该用户本期已登记此考核岗位".into(),
        ));
    }
    sqlx::query(
        // metrics_snapshot 记 source='admin'（0106 单行设计：登记与发放是同一行的
        // 状态流转，settled_at IS NULL 即未发放）。base_* 基线快照与批量分配
        // （/admin/jixiao/assign-batch）同口径——compute_metrics / jixiao_settle 的
        // 差值口径依赖它。
        "INSERT INTO jixiao_claims \
            (user_id, type_id, period, amount, metrics_snapshot, base_seed_seconds, base_uploaded, base_uploads) \
         SELECT $1, $2, $3, $4, '{\"source\":\"admin\"}'::jsonb, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = $1), 0)::bigint, \
                u.uploaded, \
                (SELECT count(*) FROM torrents tr WHERE tr.owner_id = $1 AND tr.approval_status = 1) \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .bind(amount)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.assign_jixiao",
            Some(uid),
            None,
            Some(serde_json::json!({"type_id": body.type_id,
                "period": period.clone(), "amount": amount})),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "type_id": body.type_id, "period": period }),
    ))
}

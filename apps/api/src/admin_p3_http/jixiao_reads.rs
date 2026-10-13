//! 0106 绩效考核管理端·读口（批六十六从 jixiao_ops.rs 按域拆，300 行门禁）。
//!
//! 拆出去的是 overview / payroll 两条**读口**：写口那侧要按成员逐行记审计，
//! 补完就把父文件顶过了门禁线——按本仓既有口径「拆文件」，不放宽基线。
//! 收档判据见 `scripts/pt_audit_cf_exam_audit_tiers.py`（CF2/CF4b/CF7）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

/// 全站考核总览（管理端）：按岗位聚合本期登记/达标/发薪情况，附成员明细。
/// 现状指标实时算（结算前），结算后读 metrics_at_settle 快照。
#[get("/admin/jixiao/overview")]
async fn jixiao_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-245：这页返回**每个成员的姓名 + 登记金额**（members 明细），过去只判
    // `staff()` ⇒ 91 发布员整页读走全站的钱袋子。管考核的人才有需要看它，
    // 所以接已有的 `exam.manage`（class/93），不新设键。
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE)
        .await?;
    let period = q.get("period").cloned().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });

    type Agg = (i64, String, i64, i64, i64, i64, i64, i64);
    let rows: Vec<Agg> = sqlx::query_as(
        "SELECT t.id, t.name, t.base_pay, count(c.id) FILTER (WHERE \
    c.metrics_snapshot->>'source' = 'admin'), \
                count(c.id) FILTER (WHERE c.status = 1), \
                count(c.id) FILTER (WHERE c.status = 2), \
                count(c.id) FILTER (WHERE c.status = 0 AND c.settled_at IS \
    NULL), \
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
        "SELECT c.type_id, c.user_id, u.username, c.status, c.amount, \
    c.bonus_paid, \
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
    let auth = staff(&req, &state).await?;
    // P-245：发薪明细是**别人的工资单**（username + amount + bonus + 结算时间），
    // 过去只判 `staff()`。新键 `jixiao.payroll.view` 授 class/95——与
    // sparklog.view（批六十三）、subtitles.grant（批六十四）、jixiao.assign
    // （批六十五）同档：能看全员进账的人，不该同时是能改全员进账的人以外的一堆。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::JIXIAO_PAYROLL_VIEW,
    )
    .await?;
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
        // `settled_at` 可空：本函数自己的 SQL 就写着 `ORDER BY c.settled_at
        // DESC NULLS LAST`，说明作者预期它为空，而元组按非 Option 读 ⇒ 一行
        // NULL 让整页 500（批四十三 P-172，探针号手工造的那条 status=1 且
        // settled_at IS NULL 的记录实测把发薪页打挂；生产写口都是两句同改，
        // 属纵深缺口不是现网故障）
        Option<chrono::DateTime<chrono::Utc>>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT c.id, c.user_id, u.username, t.name, c.amount, c.bonus_paid, \
                COALESCE(c.metrics_snapshot->>'settle_by', 'self') AS paid_by, \
    c.settled_at \
         FROM jixiao_claims c \
         JOIN users u ON u.id = c.user_id JOIN jixiao_types t ON t.id = \
    c.type_id \
         WHERE c.period = $1 AND c.status = 1 \
         ORDER BY c.settled_at DESC NULLS LAST LIMIT 200",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 合计要整期算：`rows` 只是 LIMIT 200 之后的一页，本期发薪超过 200 行时
    // 面板上的「本期合计」会静默偏小（批四十三 P-170：展示数取自分页后的行）
    let total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM jixiao_claims \
         WHERE period = $1 AND status = 1",
    )
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
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

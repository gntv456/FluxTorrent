use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 举报处理 ============

/// 举报队列：状态过滤 + 举报人 + 被举报对象上下文摘要
#[derive(sqlx::FromRow, serde::Serialize)]
struct ReportQueueRow {
    id: i64,
    reporter_id: i64,
    reporter_name: Option<String>,
    ref_type: String,
    ref_id: i64,
    ref_label: Option<String>,
    reason: String,
    status: i16,
    handled_name: Option<String>,
    handled_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/reports")]
async fn report_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ReportQueueQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let status_filter = q.status.as_deref().unwrap_or("pending");
    if !["pending", "handling", "handled", "all"].contains(&status_filter) {
        return Err(DomainError::Validation(
            "status 需为 pending/handling/handled/all".into(),
        ));
    }
    let rows: Vec<ReportQueueRow> = sqlx::query_as(
        "SELECT r.id, r.reporter_id, u.username AS reporter_name, r.ref_type, r.ref_id, r.reason, \
            r.status AS status, hu.username AS handled_name, r.handled_at, r.created_at, \
            cu.username AS claimed_name, \
            CASE r.ref_type \
              WHEN 'torrent' THEN (SELECT t.name FROM torrents t WHERE t.id = r.ref_id) \
              WHEN 'user' THEN (SELECT ru.username FROM users ru WHERE ru.id = r.ref_id) \
              WHEN 'comment' THEN (SELECT left(c.body, 80) FROM comments c WHERE c.id = r.ref_id) \
              /* 审计修复（P-127）：写口 /reports 对 ref_type='forum' 校验的是
                 **topics**.id，这里却按 posts.id 取正文——两张表是各自的 id 空间，
                 版主看到要么是空、要么是**另一条帖子的正文**（演示库 topics
                 13..77 与 posts 7..102 大面积重叠）。举报单的证据都不能错位。 */ \
              WHEN 'forum' THEN (SELECT tt.title FROM topics tt WHERE tt.id = r.ref_id) \
              WHEN 'subtitle' THEN (SELECT s.title FROM subtitles s WHERE s.id = r.ref_id) \
            END AS ref_label \
         FROM reports r \
         LEFT JOIN users u ON u.id = r.reporter_id \
         LEFT JOIN users hu ON hu.id = r.handled_by \
         LEFT JOIN users cu ON cu.id = r.claimed_by \
         WHERE ($1 = 'all' AND TRUE) \
            OR ($1 = 'pending' AND r.status = 0) \
            OR ($1 = 'handling' AND r.status = 2) \
            OR ($1 = 'handled' AND r.status = 1) \
         ORDER BY r.id DESC LIMIT 200",
    )
    .bind(status_filter)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ReportQueueQuery {
    #[serde(default)]
    status: Option<String>,
}

/// 认领举报（0 pending → 2 handling）：原子 CAS 防多人重复认领
#[derive(Deserialize)]
struct ReportClaimReq {
    report_id: i64,
}

#[post("/admin/reports/claim")]
async fn report_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-243：认领/放号是工单流转，版主及以上；发布员不该能占号
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::REPORTS_HANDLE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE reports SET status = 2, claimed_by = $1, claimed_at = now() \
         WHERE id = $2 AND status = 0",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "举报不存在、已被认领或已处理".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "report.claim", Some(body.report_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.report_id })))
}

/// 释放认领（2 handling → 0 pending）：认领人本人可退回队列
#[post("/admin/reports/release")]
async fn report_release(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-243：认领/放号是工单流转，版主及以上；发布员不该能占号
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::REPORTS_HANDLE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE reports SET status = 0, claimed_by = NULL, claimed_at = NULL \
         WHERE id = $1 AND status = 2 AND claimed_by = $2",
    )
    .bind(body.report_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("举报不在你名下或已结案".into()));
    }
    // 三轮遗留：释放认领与 claim/resolve 同为状态变更，补齐审计
    state
        .repo
        .audit(Some(auth.id), "report_release", Some(body.report_id))
        .await;
    Ok(ok(serde_json::json!({ "released": body.report_id })))
}

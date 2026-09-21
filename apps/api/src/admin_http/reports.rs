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
              WHEN 'forum' THEN (SELECT left(p.body, 80) FROM posts p WHERE p.id = r.ref_id) \
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

/// 处置举报：action = act(已处置) | dismiss(驳回)；结果自动 PM 通知举报人
#[derive(Deserialize)]
struct ResolveReq {
    report_id: i64,
    #[serde(default)]
    action: Option<String>, // act / dismiss，缺省 act
    #[serde(default)]
    note: String,
}

#[post("/admin/reports/resolve")]
async fn report_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let action = body.action.as_deref().unwrap_or("act");
    if !["act", "dismiss"].contains(&action) {
        return Err(DomainError::Validation("action 需为 act/dismiss".into()));
    }
    // 取举报人与对象（供 PM）
    let info: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT reporter_id, ref_type, \
         ref_id FROM reports WHERE id = $1 AND status IN (0, 2)",
    )
    .bind(body.report_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((reporter_id, ref_type, ref_id)) = info else {
        return Err(DomainError::Validation("举报不存在或已处理".into()));
    };
    sqlx::query(
        "UPDATE reports SET status = 1, handled_by = $1, handled_at = now() \
         WHERE id = $2 AND status IN (0, 2)",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // PM 通知举报人
    let subject = if action == "act" {
        "您的举报已处置"
    } else {
        "您的举报已审阅（未处置）"
    };
    let mut pm = format!(
        "您举报的对象（{} #{ref_id}）已被管理组审阅。\n\n结果：{}",
        ref_type,
        if action == "act" {
            "已按规则处置"
        } else {
            "经核实未违反规则，予以驳回"
        }
    );
    if !body.note.trim().is_empty() {
        pm.push_str("\n\n管理备注：");
        pm.push_str(body.note.trim());
    }
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(reporter_id)
    .bind(subject)
    .bind(pm)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "report.resolve", Some(body.report_id))
        .await;
    Ok(ok(
        serde_json::json!({ "resolved": body.report_id, "action": action }),
    ))
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
    Ok(ok(serde_json::json!({ "released": body.report_id })))
}

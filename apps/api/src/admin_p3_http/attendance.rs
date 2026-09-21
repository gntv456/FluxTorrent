//! P2-5 签到记录与补签
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{modify_log, staff};

// ============ P2-5 签到记录与补签 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AttendanceRow {
    user_id: i64,
    username: String,
    date: chrono::NaiveDate,
    streak: i32,
    reward: i64,
    makeup: bool,
}

#[derive(Deserialize)]
struct AttendanceQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    makeup: Option<bool>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/attendance")]
async fn admin_attendance(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AttendanceQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::ATTENDANCE_MANAGE,
    )
    .await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<AttendanceRow> = sqlx::query_as(
        r#"SELECT a.user_id, u.username, a.date, a.streak, a.reward, a.makeup
           FROM attendance a JOIN users u ON u.id = a.user_id
           WHERE ($1::bigint IS NULL OR a.user_id = $1)
             AND ($2::bool IS NULL OR a.makeup = $2)
           ORDER BY a.date DESC, a.user_id LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.makeup)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attendance a WHERE ($1::bigint IS NULL OR a.user_id = $1) \
         AND ($2::bool IS NULL OR a.makeup = $2)",
    )
    .bind(q.uid)
    .bind(q.makeup)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct MakeupReq {
    user_id: i64,
    date: chrono::NaiveDate,
}

/// 手工补签（补签卡口径）：makeup=true 落一条流水，已签日期跳过
#[post("/admin/attendance/makeup")]
async fn admin_attendance_makeup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MakeupReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::ATTENDANCE_MANAGE,
    )
    .await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
            .bind(body.user_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(body.user_id));
    }
    let n = sqlx::query(
        "INSERT INTO attendance (user_id, date, streak, reward, makeup) \
         VALUES ($1, $2, 0, 0, TRUE) ON CONFLICT (user_id, date) DO NOTHING",
    )
    .bind(body.user_id)
    .bind(body.date)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该用户当日已有签到记录".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "attendance.makeup", Some(body.user_id))
        .await;
    modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("管理补签 {}", body.date),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 撤销补签（仅可撤 makeup 行，正常签到不可删）
#[delete("/admin/attendance/makeup")]
async fn admin_attendance_makeup_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MakeupReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::ATTENDANCE_MANAGE,
    )
    .await?;
    let n = sqlx::query(
        "DELETE FROM attendance WHERE user_id = $1 AND \
         date = $2 AND makeup = TRUE",
    )
    .bind(body.user_id)
    .bind(body.date)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("无对应补签记录".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "attendance.makeup_del", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

//! P1-3 H&R 总览（参考站 user/hit-and-runs 口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{check_ids, staff};

// ============ P1-3 H&R 总览（参考站 user/hit-and-runs 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct HrRecordRow {
    user_id: i64,
    username: String,
    torrent_id: i64,
    torrent_name: Option<String>,
    /// H&R 快照按 (user_id, torrent_id) 存义务，流水从 snatches 现取；
    /// 复活/清理任务删掉 snatch 后这里就是 NULL——曾按 i64 解码导致
    /// 整个后台 H&R 页 500（一行缺流水，全表都读不出）。
    uploaded: Option<i64>,
    downloaded: Option<i64>,
    required_seconds: i32,
    seeded_seconds: i32,
    deadline: chrono::DateTime<chrono::Utc>,
    status: String,
    pardoned_name: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct HrListQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/hr/records")]
async fn hr_records(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<HrListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_VIEW)
        .await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<HrRecordRow> = sqlx::query_as(
        r#"SELECT s.user_id, u.username, s.torrent_id, t.name AS torrent_name,
                  s2.uploaded, s2.downloaded, s.required_seconds, s.seeded_seconds,
                  s.deadline, s.status, p.username AS pardoned_name, s.updated_at
           FROM hr_snapshots s
           JOIN users u ON u.id = s.user_id
           JOIN torrents t ON t.id = s.torrent_id
           LEFT JOIN snatches s2 ON s2.user_id = s.user_id AND s2.torrent_id = s.torrent_id
           LEFT JOIN users p ON p.id = s.pardoned_by
           WHERE ($1::bigint IS NULL OR s.user_id = $1)
             AND ($2::text IS NULL OR s.status = $2)
           ORDER BY s.updated_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.status.as_deref().filter(|s| !s.is_empty()))
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hr_snapshots s WHERE ($1::bigint IS NULL OR s.user_id = $1) \
         AND ($2::text IS NULL OR s.status = $2)",
    )
    .bind(q.uid)
    .bind(q.status.as_deref().filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct HrPardonItem {
    user_id: i64,
    torrent_id: i64,
}

#[derive(Deserialize)]
struct HrBatchPardonReq {
    records: Vec<HrPardonItem>,
    note: String,
}

/// 批量豁免：violated → pardoned（复用单条 hr_pardon 语义）
#[post("/admin/hr/batch-pardon")]
async fn hr_batch_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<HrBatchPardonReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON)
        .await?;
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("豁免必须填理由".into()));
    }
    check_ids(
        &body
            .records
            .iter()
            .map(|r| r.torrent_id)
            .collect::<Vec<_>>(),
    )?;
    // 判据收在 `pardon_one`（本文件上面那份）——此前这里自己写了一条
    // `status IN ('violated','open')` 的 SQL，比另两口多赦了「还没到期」的
    // 进行中义务，且理由不落库。整批先过一遍护栏，避免赦到一半被权限挡下。
    crate::services::hr_pardon::ensure_all_pardonable(
        &state,
        auth.class_id,
        &body.records.iter().map(|r| r.user_id).collect::<Vec<_>>(),
    )
    .await?;
    let mut pardoned: i64 = 0;
    for r in &body.records {
        if crate::services::hr_pardon::pardon_one(
            &state,
            auth.id,
            auth.class_id,
            "hr.batch_pardon",
            r.user_id,
            r.torrent_id,
            &body.note,
        )
        .await?
        {
            pardoned += 1;
        }
    }
    Ok(ok(serde_json::json!({ "pardoned": pardoned })))
}

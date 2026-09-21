//! 消息模板（第五轮 P2）+ 保种认领后台视图（第六轮 claims）。
//! 从 admin_p2_http.rs 按域拆出。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[get("/admin/message-templates")]
pub async fn msg_templates_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MessageTemplateRow> = sqlx::query_as(
        "SELECT id, scene_key, subject, body, note, \
         updated_at FROM message_templates ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct MsgTemplateReq {
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[put("/admin/message-templates/{id}")]
pub async fn msg_templates_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MsgTemplateReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE message_templates SET \
            subject = COALESCE($2, subject), body = COALESCE($3, body), \
            note = COALESCE($4, note), updated_at = now() \
         WHERE id = $1",
    )
    .bind(path_id)
    .bind(body.subject.clone())
    .bind(body.body.clone())
    .bind(body.note.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state
        .repo
        .audit(Some(auth.id), "msg_template.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 模板预览：以样例变量渲染场景键（QA 验证占位符替换）
#[derive(Deserialize)]
struct TemplatePreviewReq {
    scene_key: String,
    #[serde(default)]
    vars: std::collections::HashMap<String, String>,
}

#[post("/admin/message-templates/preview")]
pub async fn msg_templates_preview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TemplatePreviewReq>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let row: Option<MessageTemplateRow> = sqlx::query_as(
        "SELECT id, scene_key, subject, body, note, \
         updated_at FROM message_templates WHERE scene_key = $1",
    )
    .bind(body.scene_key.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let tpl = row.ok_or(DomainError::Validation("场景不存在".into()))?;
    let render = |mut s: String| {
        for (k, v) in &body.vars {
            s = s.replace(&format!("{{{{{k}}}}}"), v);
        }
        s
    };
    Ok(ok(serde_json::json!({
        "subject": render(tpl.subject.clone()),
        "body": render(tpl.body.clone()),
    })))
}

// ============ 第六轮：保种认领后台视图（参考站 user/claims 口径，NP claims 表） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminClaimRow {
    torrent_id: i64,
    torrent_name: Option<String>,
    seeders: i32,
    claimed_by: Option<String>,
    claimed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 认领以来做种秒数增量（snatches.seeded_seconds - 基线）
    seed_time_delta: i64,
    /// 认领以来上传量增量（字节）
    uploaded_delta: i64,
    exited_at: Option<chrono::DateTime<chrono::Utc>>,
    exit_reason: Option<String>,
}

#[derive(Deserialize)]
struct ClaimsQ {
    #[serde(default)]
    q: String,
    /// all / active / exited / unclaimed
    #[serde(default = "default_claims_state")]
    state: String,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}
fn default_claims_state() -> String {
    "all".into()
}
fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

/// 后台保种认领列表：谁认领了什么、认领以来做种时长/上传增量（NP claim.php 表格口径）
#[get("/admin/claims")]
async fn admin_claims(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ClaimsQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let mut where_parts: Vec<String> = Vec::new();
    match q.state.as_str() {
        "active" => where_parts
            .push("sp.exited_at IS NULL AND sp.claimed_by IS NOT NULL".into()),
        "exited" => where_parts.push("sp.exited_at IS NOT NULL".into()),
        "unclaimed" => where_parts
            .push("sp.exited_at IS NULL AND sp.claimed_by IS NULL".into()),
        _ => {}
    }
    if !q.q.trim().is_empty() {
        where_parts.push("(t.name ILIKE $1 OR u.username ILIKE $1)".into());
    }
    let where_sql = if where_parts.is_empty() {
        "TRUE".to_string()
    } else {
        where_parts.join(" AND ")
    };
    let sql = format!(
        r#"SELECT sp.torrent_id, t.name AS torrent_name, t.seeders,
                  u.username AS claimed_by, sp.claimed_at,
                  COALESCE(s.seeded_seconds, 0)::bigint - sp.seed_time_begin AS seed_time_delta,
                  COALESCE(s.uploaded, 0) - sp.uploaded_begin AS uploaded_delta,
                  sp.exited_at, sp.exit_reason
           FROM seed_preserve sp
           JOIN torrents t ON t.id = sp.torrent_id
           LEFT JOIN users u ON u.id = sp.claimed_by
           LEFT JOIN snatches s ON s.torrent_id = sp.torrent_id AND s.user_id = sp.claimed_by
           WHERE {where_sql}
           ORDER BY sp.claimed_at DESC NULLS LAST, sp.torrent_id
           LIMIT $2 OFFSET $3"#
    );
    let rows: Vec<AdminClaimRow> = sqlx::query_as(&sql)
        .bind(crate::http::like_pattern(&q.q))
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let count_sql = format!(
        r#"SELECT count(*) FROM seed_preserve sp
           JOIN torrents t ON t.id = sp.torrent_id
           LEFT JOIN users u ON u.id = sp.claimed_by
           WHERE {where_sql}"#
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(crate::http::like_pattern(&q.q))
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page,
    })))
}

/// 手动移出保种区（NP 后台对 claims 的管理动作；worker 只做 seeders>7 自动退出）
#[derive(Deserialize)]
struct ClaimReleaseReq {
    torrent_id: i64,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/claims/release")]
async fn admin_claim_release(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ClaimReleaseReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE seed_preserve SET exited_at = now(), exit_reason = $2 \
         WHERE torrent_id = $1 AND exited_at IS NULL",
    )
    .bind(body.torrent_id)
    .bind(body.reason.clone().unwrap_or_else(|| "manual".into()))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该种子不在保种区".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "claim.release", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "released": body.torrent_id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MessageTemplateRow {
    id: i64,
    scene_key: String,
    subject: String,
    body: String,
    note: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

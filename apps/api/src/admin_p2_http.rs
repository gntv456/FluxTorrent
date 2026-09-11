//! 第五轮 P2：置顶促销 / 自定义菜单 / 消息模板（好学站 Other 组口径）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    Ok(auth)
}

// ---- 置顶促销（sticky-promotions）----

#[derive(sqlx::FromRow, serde::Serialize)]
struct StickyPromoRow {
    id: i64,
    title: String,
    url: Option<String>,
    badge: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    enabled: bool,
    sort: i32,
}

#[get("/admin/sticky-promos")]
async fn sticky_promos_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<StickyPromoRow> = sqlx::query_as(
        "SELECT id, title, url, badge, starts_at, ends_at, enabled, sort \
         FROM sticky_promotions ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct StickyPromoReq {
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    badge: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[post("/admin/sticky-promos")]
async fn sticky_promos_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StickyPromoReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if body.title.trim().is_empty() || body.title.len() > 200 {
        return Err(DomainError::Validation("标题长度 1-200".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sticky_promotions (title, url, badge, sort, enabled, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(body.url.as_deref().map(str::trim))
    .bind(body.badge.as_deref())
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "sticky_promo.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/sticky-promos/{id}")]
async fn sticky_promos_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<StickyPromoReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE sticky_promotions SET \
            title = COALESCE($2, title), url = COALESCE($3, url), badge = COALESCE($4, badge), \
            sort = COALESCE($5, sort), enabled = COALESCE($6, enabled) \
         WHERE id = $1",
    )
    .bind(path_id)
    .bind(body.title.trim().to_string())
    .bind(body.url.clone())
    .bind(body.badge.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state.repo.audit(Some(auth.id), "sticky_promo.update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/sticky-promos/{id}")]
async fn sticky_promos_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    let n = sqlx::query("DELETE FROM sticky_promotions WHERE id = $1")
        .bind(path_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state.repo.audit(Some(auth.id), "sticky_promo.delete", None).await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

/// 前台：生效中的置顶促销（时间窗内且启用）
#[get("/sticky-promos")]
async fn sticky_promos_public(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<StickyPromoRow> = sqlx::query_as(
        "SELECT id, title, url, badge, starts_at, ends_at, enabled, sort \
         FROM sticky_promotions \
         WHERE enabled AND starts_at <= now() AND ends_at > now() \
         ORDER BY sort, id LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- 自定义菜单（menu-items）----

#[derive(sqlx::FromRow, serde::Serialize)]
struct MenuItemRow {
    id: i64,
    location: String,
    label: String,
    url: String,
    sort: i32,
    enabled: bool,
}

#[get("/admin/menu-items")]
async fn menu_items_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MenuItemRow> = sqlx::query_as(
        "SELECT id, location, label, url, sort, enabled FROM menu_items ORDER BY location, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct MenuItemReq {
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[post("/admin/menu-items")]
async fn menu_items_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MenuItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let label = body.label.as_deref().unwrap_or("").trim().to_string();
    let url = body.url.as_deref().unwrap_or("").trim().to_string();
    let location = body.location.as_deref().unwrap_or("sidebar");
    if label.is_empty() || label.len() > 50 || url.is_empty() || url.len() > 300 {
        return Err(DomainError::Validation("名称 1-50，链接 1-300".into()));
    }
    if !["sidebar", "footer", "topbar"].contains(&location) {
        return Err(DomainError::Validation("location 取值 sidebar/footer/topbar".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO menu_items (location, label, url, sort, enabled) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(location)
    .bind(label)
    .bind(url)
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "menu_item.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/menu-items/{id}")]
async fn menu_items_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MenuItemReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE menu_items SET \
            location = COALESCE($2, location), label = COALESCE($3, label), url = COALESCE($4, url), \
            sort = COALESCE($5, sort), enabled = COALESCE($6, enabled) \
         WHERE id = $1",
    )
    .bind(path_id)
    .bind(body.location.clone())
    .bind(body.label.clone())
    .bind(body.url.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state.repo.audit(Some(auth.id), "menu_item.update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/menu-items/{id}")]
async fn menu_items_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    let n = sqlx::query("DELETE FROM menu_items WHERE id = $1")
        .bind(path_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state.repo.audit(Some(auth.id), "menu_item.delete", None).await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

/// 前台：生效中的自定义菜单（按位置）
#[derive(Deserialize)]
struct MenuPublicQ {
    #[serde(default = "default_location")]
    location: String,
}
fn default_location() -> String {
    "sidebar".into()
}

#[get("/menu-items")]
async fn menu_items_public(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MenuPublicQ>,
) -> DomainResult<HttpResponse> {
    if !["sidebar", "footer", "topbar"].contains(&q.location.as_str()) {
        return Err(DomainError::Validation("location 取值 sidebar/footer/topbar".into()));
    }
    let rows: Vec<MenuItemRow> = sqlx::query_as(
        "SELECT id, location, label, url, sort, enabled FROM menu_items \
         WHERE enabled AND location = $1 ORDER BY sort, id LIMIT 30",
    )
    .bind(&q.location)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- 消息模板（message-templates）----

#[derive(sqlx::FromRow, serde::Serialize)]
struct MessageTemplateRow {
    id: i64,
    scene_key: String,
    subject: String,
    body: String,
    note: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/message-templates")]
async fn msg_templates_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MessageTemplateRow> = sqlx::query_as(
        "SELECT id, scene_key, subject, body, note, updated_at FROM message_templates ORDER BY id",
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
async fn msg_templates_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MsgTemplateReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
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
    state.repo.audit(Some(auth.id), "msg_template.update", None).await;
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
async fn msg_templates_preview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TemplatePreviewReq>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let row: Option<MessageTemplateRow> = sqlx::query_as(
        "SELECT id, scene_key, subject, body, note, updated_at FROM message_templates WHERE scene_key = $1",
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

pub fn mount_p2_tools(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(sticky_promos_list)
        .service(sticky_promos_add)
        .service(sticky_promos_update)
        .service(sticky_promos_delete)
        .service(sticky_promos_public)
        .service(menu_items_list)
        .service(menu_items_add)
        .service(menu_items_update)
        .service(menu_items_delete)
        .service(menu_items_public)
        .service(msg_templates_list)
        .service(msg_templates_update)
        .service(msg_templates_preview)
        .service(admin_claims)
        .service(admin_claim_release)
}

// ============ 第六轮：保种认领后台视图（好学站 user/claims 口径，NP claims 表） ============

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
        "active" => where_parts.push("sp.exited_at IS NULL AND sp.claimed_by IS NOT NULL".into()),
        "exited" => where_parts.push("sp.exited_at IS NOT NULL".into()),
        "unclaimed" => where_parts.push("sp.exited_at IS NULL AND sp.claimed_by IS NULL".into()),
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
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
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

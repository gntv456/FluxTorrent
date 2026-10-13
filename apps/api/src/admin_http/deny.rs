use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 第五轮：拒绝原因字典（参考站 torrent-deny-reasons 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct DenyReasonRow {
    id: i64,
    sort: i32,
    reason: String,
    enabled: bool,
}

#[get("/admin/deny-reasons")]
async fn deny_reasons_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
        let auth = staff(&req, &state).await?;
    // P-271: ledger read gated with settings.manage (as its writes).
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let rows: Vec<DenyReasonRow> = sqlx::query_as(
        "SELECT id, sort, reason, \
         enabled FROM torrent_deny_reasons ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct DenyReasonReq {
    reason: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[post("/admin/deny-reasons")]
async fn deny_reasons_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DenyReasonReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let r = body.reason.trim();
    // 按字符判长：这条文案写的是「1-200」，而理由几乎都是中文
    // （字节判长会把 67 字的中文理由自拒；P-200/203/214 同族）
    if r.is_empty() || r.chars().count() > 200 {
        return Err(DomainError::Validation("原因长度 1-200".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrent_deny_reasons (reason, sort, enabled) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(r)
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "deny_reason.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct DenyReasonPut {
    reason: Option<String>,
    sort: Option<i32>,
    enabled: Option<bool>,
}

#[put("/admin/deny-reasons/{id}")]
async fn deny_reasons_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<DenyReasonPut>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let rid = path.into_inner();
    if let Some(r) = body.reason.as_deref() {
        if r.trim().is_empty() || r.chars().count() > 200 {
            return Err(DomainError::Validation("原因长度 1-200".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE torrent_deny_reasons SET \
            reason = COALESCE($2, reason), sort = COALESCE($3, sort), enabled = COALESCE($4, enabled) \
         WHERE id = $1",
    )
    .bind(rid)
    .bind(body.reason.as_deref().map(str::trim))
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("拒绝原因不存在".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "deny_reason.update",
            Some(rid),
            None,
            Some(serde_json::json!({
                "reason": body.reason.as_deref().map(str::trim),
                "enabled": body.enabled,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/deny-reasons/{id}")]
async fn deny_reasons_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let rid = path.into_inner();
    // 审计修复（P1）：被种子引用（torrents.deny_reason_id del=a FK）时删除必 500。
    // 与 category_delete 同款前置护栏：有引用先解绑/换用别的理由。
    // 护栏自己不许吞错：查询失败时旧写法把引用数当成 0 ⇒ 直接放行删除
    // （本库该外键是 NO ACTION，最坏从「友好 400」退化成 500；若是 SET NULL
    //  就会静默解绑历史种子的驳回理由——同一形状不能留给运气）
    let refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE deny_reason_id = $1",
    )
    .bind(rid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if refs > 0 {
        return Err(DomainError::Validation(format!(
            "仍有 {refs} 个种子使用该拒绝理由（含已删除种子），请先改用其他理由"
        )));
    }
    // 先读后删：行没了以后 reason 文本是「删掉了哪条理由」的唯一出处
    let gone: Option<String> = sqlx::query_scalar(
        "SELECT reason FROM torrent_deny_reasons WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let gone = match gone {
        Some(t) => t,
        None => return Err(DomainError::Validation("拒绝原因不存在".into())),
    };
    let n = sqlx::query("DELETE FROM torrent_deny_reasons WHERE id = $1")
        .bind(rid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("拒绝原因不存在".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "deny_reason.delete",
            Some(rid),
            None,
            Some(serde_json::json!({ "reason": gone })),
        )
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

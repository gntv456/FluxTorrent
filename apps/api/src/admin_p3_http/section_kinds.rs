//! P3-12 Section 维度 kinds CRUD（section_kinds 表）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::section_public::regex_check_kind;
use super::sections::LEGACY_KINDS;
use super::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct SectionKindRow {
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) sort: i32,
}

#[derive(Deserialize)]
struct SectionKindReq {
    kind: String,
    label: String,
    #[serde(default)]
    sort: Option<i32>,
}

/// 质量维度元数据 CRUD（0085，NP 自定义 Section 口径）：
/// 站方自建维度（如 resolution/语言），字典行挂维度下，发布表单动态渲染
#[get("/admin/section-kinds")]
async fn section_kinds_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let rows: Vec<SectionKindRow> = sqlx::query_as(
        "SELECT kind, label, sort FROM section_kinds ORDER BY sort, kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[post("/admin/section-kinds")]
async fn section_kinds_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionKindReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let kind = body.kind.trim().to_lowercase();
    if !LEGACY_KINDS.contains(&kind.as_str()) && !regex_check_kind(&kind) {
        return Err(DomainError::Validation(
            "维度标识需为小写字母开头的 [a-z0-9_]（≤32 字符）".into(),
        ));
    }
    if LEGACY_KINDS.contains(&kind.as_str()) {
        return Err(DomainError::Validation("该维度已内置".into()));
    }
    if body.label.trim().is_empty() {
        return Err(DomainError::Validation("显示名称不能为空".into()));
    }
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_kinds WHERE kind = $1)",
    )
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if dup {
        return Err(DomainError::Validation("维度标识已存在".into()));
    }
    sqlx::query("INSERT INTO section_kinds (kind, label, sort) VALUES ($1, $2, COALESCE($3, 0))")
        .bind(&kind)
        .bind(body.label.trim())
        .bind(body.sort)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_kind.add", None)
        .await;
    Ok(ok(serde_json::json!({ "kind": kind })))
}

#[put("/admin/section-kinds/{kind}")]
async fn section_kinds_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<SectionKindReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let kind = path.into_inner();
    if body.label.trim().is_empty() {
        return Err(DomainError::Validation("显示名称不能为空".into()));
    }
    let n = sqlx::query(
        "UPDATE section_kinds SET label = $2, sort = COALESCE($3, sort) WHERE kind = $1",
    )
    .bind(&kind)
    .bind(body.label.trim())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(Some(auth.id), "section_kind.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/section-kinds/{kind}")]
async fn section_kinds_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let kind = path.into_inner();
    if LEGACY_KINDS.contains(&kind.as_str()) {
        return Err(DomainError::Validation("内置维度不可删除".into()));
    }
    // 删除会级联清空字典与种子归属，先挡在用中的维度
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrent_sections ts JOIN section_dict sd ON sd.id = ts.dict_id \
         WHERE sd.kind = $1",
    )
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if used > 0 {
        return Err(DomainError::Validation(
            "该维度下已有种子在用，不能删除".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
        .bind(&kind)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(Some(auth.id), "section_kind.del", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": kind })))
}

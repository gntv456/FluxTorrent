//! P3-12 Section 维度字典 CRUD（section_dict 表：九维统一通道）+ 管理端统一读取。
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::{is_custom_kind, staff};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct SectionDictRow {
    pub(super) id: i64,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) sort: i32,
    pub(super) mode_id: Option<i32>,
}

#[derive(Deserialize)]
struct SectionDictQ {
    #[serde(default)]
    kind: Option<String>,
}

/// 维度字典统一读取（管理端全量；发布表单用 /section-dict 公开读）
#[get("/admin/section-dict")]
async fn section_dict_admin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SectionDictQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<SectionDictRow> =
        section_dict_rows(&state.repo.db, q.kind.as_deref()).await?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

pub(super) async fn section_dict_rows(
    db: &sqlx::PgPool,
    kind: Option<&str>,
) -> DomainResult<Vec<SectionDictRow>> {
    // 0087：media/grades/editions 字典行已入 section_dict，统一读取（不再回落实体表）
    sqlx::query_as(
        "SELECT id, kind, name, sort, mode_id FROM section_dict \
         WHERE ($1::text IS NULL OR kind = $1) ORDER BY kind, sort, id",
    )
    .bind(kind)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

#[derive(Deserialize)]
struct SectionDictReq {
    kind: String,
    name: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    mode_id: Option<i32>,
}

#[post("/admin/section-dict")]
async fn section_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let kind = body.kind.as_str();
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称不能为空".into()));
    }
    if !is_custom_kind(&state.repo.db, kind).await {
        return Err(DomainError::Validation(
            "未知维度：请先在「维度管理」中创建该维度".into(),
        ));
    }
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_dict WHERE kind = $1 AND \
         lower(trim(name)) = lower(trim($2)))",
    )
    .bind(kind)
    .bind(body.name.trim())
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        // 无查重时后台可以无限堆同名选项，筛选与统计口径都会分裂
        return Err(DomainError::Validation("该维度下已存在同名选项".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO section_dict (kind, name, sort, mode_id) VALUES \
         ($1, $2, COALESCE($3, 0), $4) RETURNING id",
    )
    .bind(kind)
    .bind(body.name.trim())
    .bind(body.sort)
    .bind(body.mode_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_dict.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/section-dict/{id}")]
async fn section_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<SectionDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let kind = body.kind.as_str();
    if !is_custom_kind(&state.repo.db, kind).await {
        return Err(DomainError::Validation("未知维度".into()));
    }
    let n = sqlx::query(
        "UPDATE section_dict SET name = $2, sort = COALESCE($3, sort), \
         mode_id = $4 WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.sort)
    .bind(body.mode_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "section_dict.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/section-dict/{id}")]
async fn section_dict_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<SectionDictQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let kind = q.kind.clone().unwrap_or_default();
    if !is_custom_kind(&state.repo.db, &kind).await {
        return Err(DomainError::Validation("未知维度".into()));
    }
    // 必须校验 id 真属于 query 里的 kind：否则 ?kind=media 能删掉 grades 的选项
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $1 AND kind = $2)",
    )
    .bind(id)
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !owned {
        return Err(DomainError::Validation("该选项不属于此维度".into()));
    }
    // torrent_sections.dict_id 是 ON DELETE CASCADE：删一个选项会静默抹掉全站
    // 种子在该维度上的归属，所以比照「维度管理」的口径先挡住在用项
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrent_sections WHERE dict_id = $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if used > 0 {
        return Err(DomainError::Validation(format!(
            "已有 {used} 个种子挂在该选项上，请先改挂或删除这些种子"
        )));
    }
    let n = sqlx::query("DELETE FROM section_dict WHERE id = $1 AND kind = $2")
        .bind(id)
        .bind(&kind)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "section_dict.del", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

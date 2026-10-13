//! P3-12 Section 维度 kinds CRUD（section_kinds 表）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::section_public::regex_check_kind;
use super::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct SectionKindRow {
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) sort: i32,
    /// B2（0195）：六类型字段系统
    pub(super) field_type: String,
    pub(super) required: bool,
    pub(super) multiple: bool,
    pub(super) enabled: bool,
    pub(super) icon_key: Option<String>,
    pub(super) bg_color: Option<String>,
    /// 该维度下已挂的字典项数（枚举维度用；自由值维度恒 0）
    pub(super) options: i64,
}

#[derive(Deserialize)]
struct SectionKindReq {
    kind: String,
    label: String,
    #[serde(default)]
    sort: Option<i32>,
    /// 六类型之一；缺省 select（存量语义不变）
    #[serde(default)]
    field_type: Option<String>,
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    multiple: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    icon_key: Option<String>,
    #[serde(default)]
    bg_color: Option<String>,
}

/// 质量维度元数据 CRUD（0085，NP 自定义 Section 口径）：
/// 站方自建维度（如 resolution/语言），字典行挂维度下，发布表单动态渲染。
/// B2（0195）：扩为六类型字段系统（text/number/select/multiselect/date/bool）
/// + 必填/多值/启用开关，与用户侧自定义字段同一套语义。
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
        "SELECT k.kind, k.label, k.sort, k.field_type, k.required, k.multiple, \
                k.enabled, k.icon_key, k.bg_color, \
                (SELECT count(*) FROM section_dict d WHERE d.kind = k.kind)::bigint \
                    AS options \
         FROM section_kinds k ORDER BY k.sort, k.kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    serde_json::to_value(rows)
        .map_err(|e| DomainError::Internal(e.into()))
        .map(ok)
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
    if !regex_check_kind(&kind) {
        return Err(DomainError::Validation(
            "维度标识需为小写字母开头的 [a-z0-9_]（≤32 字符）".into(),
        ));
    }
    if body.label.trim().is_empty() {
        return Err(DomainError::Validation("显示名称不能为空".into()));
    }
    // B2：字段类型必须合法（与 user_field_defs.type 同一取值集合）
    let field_type = body
        .field_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("select");
    if !crate::fields::valid_field_type(field_type) {
        return Err(DomainError::Validation(format!(
            "未知字段类型 {field_type}（可选：text/number/select/multiselect/date/bool）"
        )));
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
    sqlx::query(
        "INSERT INTO section_kinds \
           (kind, label, sort, field_type, required, multiple, enabled, \
            icon_key, bg_color) \
         VALUES ($1, $2, COALESCE($3, 0), $4, COALESCE($5, FALSE), \
                 COALESCE($6, FALSE), COALESCE($7, TRUE), $8, $9)",
    )
    .bind(&kind)
    .bind(body.label.trim())
    .bind(body.sort)
    .bind(field_type)
    .bind(body.required)
    .bind(body.multiple)
    .bind(body.enabled)
    .bind(
        body.icon_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        body.bg_color
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(Some(auth.id), "section_kind.add", None, None,
            Some(serde_json::json!({"kind": kind})))
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
    // B2：字段类型**不可改**——已建维度的存量值按旧类型解释，改类型会让历史值语义
    // 失效（如 select→number 后 dict_id 行无法解释）。给了不同的值就直接拒收，
    // 而不是静默忽略（静默忽略会让站长以为改成功了）。
    if let Some(ft) = body.field_type.as_deref().map(str::trim) {
        if !ft.is_empty() {
            let cur: Option<String> = sqlx::query_scalar(
                "SELECT field_type FROM section_kinds WHERE kind = $1",
            )
            .bind(&kind)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(cur) = cur else {
                return Err(DomainError::NotFound(0));
            };
            if cur != ft {
                return Err(DomainError::Validation(
                    "字段类型不可修改（存量值按原类型解释）；如需变更请新建维度".into(),
                ));
            }
        }
    }
    let n = sqlx::query(
        "UPDATE section_kinds SET label = $2, sort = COALESCE($3, sort), \
           required = COALESCE($4, required), \
           multiple = COALESCE($5, multiple), \
           enabled  = COALESCE($6, enabled), \
           icon_key = COALESCE($7, icon_key), \
           bg_color = COALESCE($8, bg_color) \
         WHERE kind = $1",
    )
    .bind(&kind)
    .bind(body.label.trim())
    .bind(body.sort)
    .bind(body.required)
    .bind(body.multiple)
    .bind(body.enabled)
    .bind(
        body.icon_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        body.bg_color
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit_detail(Some(auth.id), "section_kind.update", None, None,
            Some(serde_json::json!({"kind": kind})))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

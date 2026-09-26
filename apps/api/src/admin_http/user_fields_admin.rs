//! G4（2026-09-26）：运营侧对「用户自定义字段值」的查看与代改。
//!
//! 字段定义（defs）与本人读写仍在 `auth_http::user_fields`；这里补后台两件事，
//! 权限同为 `userfields.manage`（能管字段定义的人才能改值）：
//!   · GET /admin/users/{id}/fields  目标用户全字段 + 值（含 private/停用，
//!     排查「用户说填了却看不到」时一次看全）
//!   · PUT /admin/users/{id}/fields  代改（`null` = 清空，required 同本人口径
//!     不可清；改动按 def 当前类型走 `crate::fields` 同源校验）
//! 「按字段值筛人」在 user_list.rs 的 where_parts 里（与列表查询同源）。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserFieldRow {
    key: String,
    label: String,
    r#type: String,
    required: bool,
    visibility: String,
    options: serde_json::Value,
    enabled: bool,
    value: Option<serde_json::Value>,
}

async fn user_exists(state: &AppState, uid: i64) -> DomainResult<()> {
    let ok: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
            .bind(uid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(internal)?;
    if ok {
        Ok(())
    } else {
        Err(DomainError::NotFound(uid))
    }
}

#[get("/admin/users/{id}/fields")]
async fn admin_user_fields(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let uid = path.into_inner();
    user_exists(&state, uid).await?;
    let rows: Vec<AdminUserFieldRow> = sqlx::query_as(
        "SELECT d.key, d.label, d.type, d.required, d.visibility, \
             d.options, d.enabled, v.value \
         FROM user_field_defs d \
         LEFT JOIN user_field_values v \
           ON v.field_key = d.key AND v.user_id = $1 \
         ORDER BY d.sort, d.key",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AdminFieldValuesBody {
    values: std::collections::HashMap<String, serde_json::Value>,
}

/// 代改。只收**启用中**字段：停用/不存在的 key 直接 400，不静默跳过——
/// 后台是显式表单，静默丢弃会让运营以为改成功了（本人写入口的静默口径保留）。
#[put("/admin/users/{id}/fields")]
async fn admin_user_fields_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AdminFieldValuesBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let uid = path.into_inner();
    user_exists(&state, uid).await?;
    if body.values.is_empty() {
        return Err(DomainError::Validation("缺少要改的字段".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    for (key, val) in &body.values {
        let def: Option<(String, String, bool, serde_json::Value)> =
            sqlx::query_as(
                "SELECT type, label, required, options FROM user_field_defs \
                 WHERE key = $1 AND enabled",
            )
            .bind(key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
        let Some((ftype, label, need, options)) = def else {
            return Err(DomainError::Validation(format!(
                "字段「{key}」不存在或已停用"
            )));
        };
        if val.is_null() {
            if need {
                return Err(DomainError::Validation(format!(
                    "「{label}」为必填字段，不能清空"
                )));
            }
            sqlx::query(
                "DELETE FROM user_field_values WHERE user_id = $1 AND \
                 field_key = $2",
            )
            .bind(uid)
            .bind(key)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            continue;
        }
        crate::fields::validate_value(&ftype, &options, val)
            .map_err(DomainError::Validation)?;
        sqlx::query(
            "INSERT INTO user_field_values (user_id, field_key, value) \
             VALUES ($1, $2, $3) ON CONFLICT (user_id, field_key) \
             DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
        )
        .bind(uid)
        .bind(key)
        .bind(val)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    state
        .repo
        .audit(Some(auth.id), "user.fields_update", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "saved": body.values.len() })))
}

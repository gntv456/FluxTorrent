//! 用户自定义字段的**定义侧 CRUD**（批三十九从 user_fields.rs 拆出，
//! 300 行门禁：该文件是存量超限项，只许瘦不许胖）。
//!
//! 这里管「站长怎么定义字段」：列表 / 新增 / 改定义（类型不可改）/
//! 删除（有值要 confirm，values 随 FK CASCADE 一起走）。
//! 值形状与类型语义在 `crate::fields`，入参合法性在 `user_fields_def`，
//! 本人读写与公开下发仍在 `user_fields.rs`。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::user_fields::internal;
use super::user_fields_def::{
    norm_module_key, valid_key, validate_def, UserFieldDefBody,
};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 后台：字段定义 CRUD（抄 medal_rarities 作业） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct UserFieldDefRow {
    key: String,
    label: String,
    r#type: String,
    required: bool,
    visibility: String,
    show_on_register: bool,
    options: serde_json::Value,
    sort: i32,
    enabled: bool,
    /// 挂到某模块：模块关闭时字段从前台（注册页/usercp/公开档案）下线（0199）
    module_key: Option<String>,
    /// 已填用户数（删字段前的心里有数）
    filled: i64,
}

#[get("/admin/user-fields")]
pub async fn user_fields_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let rows: Vec<UserFieldDefRow> = sqlx::query_as(
        "SELECT d.key, d.label, d.type, d.required, d.visibility, \
             d.show_on_register, d.options, d.sort, d.enabled, d.module_key, \
             (SELECT count(*) FROM user_field_values v \
               WHERE v.field_key = d.key) AS filled \
         FROM user_field_defs d ORDER BY d.sort, d.key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[post("/admin/user-fields/{key}")]
pub async fn user_fields_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<UserFieldDefBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let key = path.into_inner();
    if !valid_key(&key) {
        return Err(DomainError::Validation(
            "key 需为小写字母/数字/下划线，≤40 字符".into(),
        ));
    }
    validate_def(&body)?;
    crate::modules::require_known_module(
        &state.repo.db,
        norm_module_key(&body.module_key),
    )
    .await?;
    sqlx::query(
        "INSERT INTO user_field_defs (key, label, type, required, \
         visibility, show_on_register, options, sort, enabled, module_key) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&key)
    .bind(body.label.trim())
    .bind(&body.r#type)
    .bind(body.required)
    .bind(&body.visibility)
    .bind(body.show_on_register)
    .bind(&body.options)
    .bind(body.sort)
    .bind(body.enabled)
    .bind(norm_module_key(&body.module_key))
    .execute(&state.repo.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            DomainError::Validation("字段 key 已存在".into())
        }
        _ => internal(e),
    })?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user_field_add",
            None,
            None,
            // P-141（批三十九）：ref 要带得上「动的是哪条字段」。旧写法三处
            // 都传 None ⇒ 审计里只剩「有人改过字段定义」，而删字段是连带
            // 清掉全站用户值的不可逆动作，事后查不到是哪一条。
            Some(serde_json::json!({ "key": &key })),
        )
        .await;
    Ok(ok(serde_json::json!({ "added": key })))
}

#[put("/admin/user-fields/{key}")]
pub async fn user_fields_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<UserFieldDefBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let key = path.into_inner();
    validate_def(&body)?;
    crate::modules::require_known_module(
        &state.repo.db,
        norm_module_key(&body.module_key),
    )
    .await?;
    // type 不可改（存量 values 的形状解释会失配）；options 收紧会留死值，
    // 允许改但已在值集外的旧值照存（展示原样）
    let n = sqlx::query(
        "UPDATE user_field_defs SET label = $2, required = $3, \
         visibility = $4, show_on_register = $5, options = $6, sort = $7, \
         enabled = $8, module_key = $10 WHERE key = $1 AND type = $9",
    )
    .bind(&key)
    .bind(body.label.trim())
    .bind(body.required)
    .bind(&body.visibility)
    .bind(body.show_on_register)
    .bind(&body.options)
    .bind(body.sort)
    .bind(body.enabled)
    .bind(&body.r#type)
    .bind(norm_module_key(&body.module_key))
    .execute(&state.repo.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if n == 0 {
        // 区分：不存在 or type 改动
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_field_defs WHERE key = $1)",
        )
        .bind(&key)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        return Err(if exists {
            DomainError::Validation("字段类型不可修改（存量值形状失配）".into())
        } else {
            DomainError::Validation("字段不存在".into())
        });
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user_field_update",
            None,
            None,
            Some(serde_json::json!({ "key": &key })),
        )
        .await;
    Ok(ok(serde_json::json!({ "updated": key })))
}

#[derive(Deserialize)]
struct FieldDeleteBody {
    /// 已有值的字段需显式确认才可删（连带删 values）
    #[serde(default)]
    confirm_drop_values: bool,
}

#[post("/admin/user-fields/{key}/delete")]
pub async fn user_fields_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<FieldDeleteBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USERFIELDS_MANAGE,
    )
    .await?;
    let key = path.into_inner();
    let filled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_field_values WHERE field_key = $1",
    )
    .bind(&key)
    .fetch_one(&state.repo.db)
    .await
    .map_err(internal)?;
    if filled > 0 && !body.confirm_drop_values {
        return Err(DomainError::Validation(format!(
            "该字段已有 {filled} 个用户的值；确认删除请传 confirm_drop_values=true"
        )));
    }
    let n = sqlx::query("DELETE FROM user_field_defs WHERE key = $1")
        .bind(&key)
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("字段不存在".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user_field_delete",
            None,
            None,
            // 删除是连带清全站用户值的不可逆动作：审计要答得出
            // 「删了哪条、当时有多少人的值被清」
            Some(serde_json::json!({ "key": &key, "dropped_values": filled })),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "deleted": key, "dropped_values": filled }),
    ))
}

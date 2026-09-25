//! 用户自定义字段（一审 R4.1）：defs 后台 CRUD + 用户侧读写 + 公开档案下发。
//! 数据模型见 0186：defs（站长定义）+ values（按用户）。字段值类型解释在 def
//! 侧（text/number/select/multiselect/date/bool），values.value 统一 JSONB。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use super::user_fields_def::{
    norm_module_key, valid_key, validate_def, UserFieldDefBody,
};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 校验值形状与 def 类型匹配；select/multiselect 额外校验选项在 options 内。
///
/// B 批（2026-09-25）：实现已搬到 `crate::fields::validate_value`，与内容侧自定义维度
/// **共用同一套类型语义**（六类型逐字对齐），避免两边各自漂移。此处保留原签名转发，
/// register.rs 等既有调用点零改动。
pub(crate) fn validate_value(
    def_type: &str,
    options: &serde_json::Value,
    v: &serde_json::Value,
) -> Result<(), String> {
    crate::fields::validate_value(def_type, options, v)
}

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
        crate::authz::perm::SITEPACKS_MANAGE,
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
        crate::authz::perm::SITEPACKS_MANAGE,
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
        .audit(Some(auth.id), "user_field_add", None)
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
        crate::authz::perm::SITEPACKS_MANAGE,
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
        .audit(Some(auth.id), "user_field_update", None)
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
        crate::authz::perm::SITEPACKS_MANAGE,
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
        .audit(Some(auth.id), "user_field_delete", None)
        .await;
    Ok(ok(
        serde_json::json!({ "deleted": key, "dropped_values": filled }),
    ))
}

// ============ 用户侧：本人字段读写（usercp 资料编辑） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MyFieldRow {
    key: String,
    label: String,
    r#type: String,
    required: bool,
    options: serde_json::Value,
    value: Option<serde_json::Value>,
}

/// GET /me/fields：字段定义（enabled）+ 当前用户的值
#[get("/me/fields")]
pub async fn my_fields(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyFieldRow> = sqlx::query_as::<_, MyFieldRow>(&format!(
        "SELECT d.key, d.label, d.type, d.required, d.options, v.value \
         FROM user_field_defs d \
         LEFT JOIN user_field_values v \
           ON v.field_key = d.key AND v.user_id = $1 \
         WHERE d.enabled AND {} \
         ORDER BY d.sort, d.key",
        crate::modules::module_on_sql("d")
    ))
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct MyFieldsPutBody {
    values: std::collections::HashMap<String, serde_json::Value>,
}

/// PUT /me/fields：批量保存本人字段值（只收 enabled 字段；逐字段校验形状）
#[put("/me/fields")]
pub async fn my_fields_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MyFieldsPutBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let mut saved = 0i64;
    for (key, val) in &body.values {
        let def: Option<(String, String, bool, serde_json::Value)> =
            sqlx::query_as::<_, (String, String, bool, serde_json::Value)>(
                &format!(
                "SELECT type, label, required, options FROM user_field_defs \
             WHERE key = $1 AND enabled AND {}",
                crate::modules::module_on_sql("")
            ),
            )
            .bind(key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
        let Some((ftype, _label, _req, options)) = def else {
            continue; // 未知/停用字段静默跳过
        };
        if val.is_null() {
            // null = 清除该字段值（required 字段不允许清）
            if _req {
                return Err(DomainError::Validation(format!(
                    "「{_label}」为必填字段，不能清空"
                )));
            }
            sqlx::query(
                "DELETE FROM user_field_values WHERE user_id = $1 AND \
                 field_key = $2",
            )
            .bind(auth.id)
            .bind(key)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            saved += 1;
            continue;
        }
        validate_value(&ftype, &options, val)
            .map_err(DomainError::Validation)?;
        sqlx::query(
            "INSERT INTO user_field_values (user_id, field_key, value) \
             VALUES ($1, $2, $3) ON CONFLICT (user_id, field_key) \
             DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
        )
        .bind(auth.id)
        .bind(key)
        .bind(val)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        saved += 1;
    }
    tx.commit().await.map_err(internal)?;
    Ok(ok(serde_json::json!({ "saved": saved })))
}

/// 公开档案下发（profile.rs 调用）：public 字段 + 有值用户的值
pub(crate) async fn public_fields_for(
    db: &PgPool,
    uid: i64,
) -> Vec<serde_json::Value> {
    let rows: Vec<(String, String, String, serde_json::Value)> =
        sqlx::query_as::<_, (String, String, String, serde_json::Value)>(
            &format!(
                "SELECT d.key, d.label, d.type, v.value \
             FROM user_field_defs d \
             JOIN user_field_values v ON v.field_key = d.key \
             WHERE d.enabled AND d.visibility = 'public' \
               AND {} \
               AND v.user_id = $1 \
             ORDER BY d.sort, d.key",
                crate::modules::module_on_sql("d")
            ),
        )
        .bind(uid)
        .fetch_all(db)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|(key, label, ftype, value)| {
            serde_json::json!({
                "key": key, "label": label, "type": ftype, "value": value,
            })
        })
        .collect()
}

/// 注册页字段下发（register 前拉取）：show_on_register 且 enabled
pub(crate) async fn register_fields(db: &PgPool) -> Vec<serde_json::Value> {
    let rows: Vec<(String, String, String, bool, serde_json::Value)> =
        sqlx::query_as::<_, (String, String, String, bool, serde_json::Value)>(
            &format!(
            "SELECT key, label, type, required, options FROM user_field_defs \
             WHERE enabled AND show_on_register AND {} ORDER BY sort, key",
            crate::modules::module_on_sql("")
        ),
        )
        .fetch_all(db)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|(key, label, ftype, required, options)| {
            serde_json::json!({
                "key": key, "label": label, "type": ftype,
                "required": required, "options": options,
            })
        })
        .collect()
}

/// register.rs 用的公开别名（required 校验共用同一形状校验器）
pub(crate) fn validate_value_pub(
    def_type: &str,
    options: &serde_json::Value,
    v: &serde_json::Value,
) -> Result<(), String> {
    validate_value(def_type, options, v)
}

/// GET /register-fields：注册页动态字段下发（无登录态，公开——字段定义
/// 本身不是敏感信息；值校验在注册提交时做）
#[get("/register-fields")]
pub async fn register_fields_endpoint(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    Ok(ok(register_fields(&state.repo.db).await))
}

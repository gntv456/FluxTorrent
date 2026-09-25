//! 用户自定义字段（一审 R4.1）：defs 后台 CRUD + 用户侧读写 + 公开档案下发。
//! 数据模型见 0186：defs（站长定义）+ values（按用户）。字段值类型解释在 def
//! 侧（text/number/select/multiselect/date/bool），values.value 统一 JSONB。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 40
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn valid_field_type(t: &str) -> bool {
    matches!(
        t,
        "text" | "number" | "select" | "multiselect" | "date" | "bool"
    )
}

/// 校验值形状与 def 类型匹配；select/multiselect 额外校验选项在 options 内
pub(crate) fn validate_value(
    def_type: &str,
    options: &serde_json::Value,
    v: &serde_json::Value,
) -> Result<(), String> {
    let allowed: Vec<String> = options
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    o.get("value")
                        .and_then(serde_json::Value::as_str)
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    match def_type {
        "text" => {
            v.as_str()
                .filter(|s| s.len() <= 500)
                .map(|_| ())
                .ok_or("text 字段需为 ≤500 字符字符串")?;
        }
        "number" => {
            v.as_f64().map(|_| ()).ok_or("number 字段需为数字")?;
        }
        "date" => {
            let s = v
                .as_str()
                .ok_or("date 字段需为 YYYY-MM-DD 字符串")?;
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|_| ())
                .map_err(|_| "date 字段需为 YYYY-MM-DD".to_string())?;
        }
        "bool" => {
            v.as_bool()
                .map(|_| ())
                .ok_or("bool 字段需为 true/false")?;
        }
        "select" => {
            let s = v.as_str().ok_or("select 字段需为选项值字符串")?;
            if !allowed.contains(&s.to_string()) {
                return Err("值不在字段选项集内".into());
            }
        }
        "multiselect" => {
            let arr = v
                .as_array()
                .ok_or("multiselect 字段需为选项值数组")?;
            if arr.len() > 20 {
                return Err("multiselect 最多 20 项".into());
            }
            for item in arr {
                let s = item
                    .as_str()
                    .ok_or("multiselect 数组元素需为字符串")?;
                if !allowed.contains(&s.to_string()) {
                    return Err("值不在字段选项集内".into());
                }
            }
        }
        _ => return Err("未知字段类型".into()),
    }
    Ok(())
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
             d.show_on_register, d.options, d.sort, d.enabled, \
             (SELECT count(*) FROM user_field_values v \
               WHERE v.field_key = d.key) AS filled \
         FROM user_field_defs d ORDER BY d.sort, d.key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct UserFieldDefBody {
    label: String,
    r#type: String,
    #[serde(default)]
    required: bool,
    #[serde(default = "default_vis")]
    visibility: String,
    #[serde(default)]
    show_on_register: bool,
    #[serde(default)]
    options: serde_json::Value,
    #[serde(default = "default_sort")]
    sort: i32,
    #[serde(default = "default_true")]
    enabled: bool,
}
fn default_vis() -> String {
    "public".into()
}
fn default_sort() -> i32 {
    100
}
fn default_true() -> bool {
    true
}

fn validate_def(body: &UserFieldDefBody) -> DomainResult<()> {
    if body.label.trim().is_empty() || body.label.len() > 50 {
        return Err(DomainError::Validation("字段名需 1-50 字符".into()));
    }
    if !valid_field_type(&body.r#type) {
        return Err(DomainError::Validation(
            "type 需为 text/number/select/multiselect/date/bool".into(),
        ));
    }
    if !["public", "private"].contains(&body.visibility.as_str()) {
        return Err(DomainError::Validation(
            "visibility 需为 public/private".into(),
        ));
    }
    if matches!(body.r#type.as_str(), "select" | "multiselect") {
        let arr = body
            .options
            .as_array()
            .ok_or_else(|| {
                DomainError::Validation("select 类型需提供 options 数组".into())
            })?;
        if arr.is_empty() || arr.len() > 50 {
            return Err(DomainError::Validation(
                "options 需 1-50 项".into(),
            ));
        }
        for o in arr {
            let val = o
                .get("value")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    DomainError::Validation(
                        "options 项需含 value 字符串".into(),
                    )
                })?;
            if val.is_empty() || val.len() > 100 {
                return Err(DomainError::Validation(
                    "option value 需 1-100 字符".into(),
                ));
            }
        }
    }
    Ok(())
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
    sqlx::query(
        "INSERT INTO user_field_defs (key, label, type, required, \
         visibility, show_on_register, options, sort, enabled) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
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
    .execute(&state.repo.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db)
            if db.constraint().is_some() =>
        {
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
    // type 不可改（存量 values 的形状解释会失配）；options 收紧会留死值，
    // 允许改但已在值集外的旧值照存（展示原样）
    let n = sqlx::query(
        "UPDATE user_field_defs SET label = $2, required = $3, \
         visibility = $4, show_on_register = $5, options = $6, sort = $7, \
         enabled = $8 WHERE key = $1 AND type = $9",
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
    Ok(ok(serde_json::json!({ "deleted": key, "dropped_values": filled })))
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
    let rows: Vec<MyFieldRow> = sqlx::query_as(
        "SELECT d.key, d.label, d.type, d.required, d.options, v.value \
         FROM user_field_defs d \
         LEFT JOIN user_field_values v \
           ON v.field_key = d.key AND v.user_id = $1 \
         WHERE d.enabled \
         ORDER BY d.sort, d.key",
    )
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
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    let mut saved = 0i64;
    for (key, val) in &body.values {
        let def: Option<(
            String,
            String,
            bool,
            serde_json::Value,
        )> = sqlx::query_as(
            "SELECT type, label, required, options FROM user_field_defs \
             WHERE key = $1 AND enabled",
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
        sqlx::query_as(
            "SELECT d.key, d.label, d.type, v.value \
             FROM user_field_defs d \
             JOIN user_field_values v ON v.field_key = d.key \
             WHERE d.enabled AND d.visibility = 'public' \
               AND v.user_id = $1 \
             ORDER BY d.sort, d.key",
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
pub(crate) async fn register_fields(
    db: &PgPool,
) -> Vec<serde_json::Value> {
    let rows: Vec<(
        String,
        String,
        String,
        bool,
        serde_json::Value,
    )> = sqlx::query_as(
        "SELECT key, label, type, required, options FROM user_field_defs \
         WHERE enabled AND show_on_register ORDER BY sort, key",
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

pub fn mount_user_fields(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(user_fields_list)
        .service(user_fields_add)
        .service(user_fields_update)
        .service(user_fields_delete)
        .service(my_fields)
        .service(my_fields_put)
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

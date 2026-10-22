//! 用户自定义字段（一审 R4.1）：用户侧读写 + 公开档案下发 + 注册页下发。
//! 数据模型见 0186：defs（站长定义）+ values（按用户）。字段值类型解释在 def
//! 侧（text/number/select/multiselect/date/bool），values.value 统一 JSONB。
//! 站长侧的 defs CRUD 在 `user_fields_crud.rs`（本文件是存量超限项，只许瘦）。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::fields::named;
use crate::http::require_auth;
use crate::state::AppState;

pub(crate) fn internal(e: sqlx::Error) -> DomainError {
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
    // P-139（批三十九）：一次提交的字段条数要有上限。旧写法无界——
    // 每个 key 都要跑一次 defs 查询（停用/未知也要查一次），
    // 300 个乱码 key 就是一次 300 查的打库请求（同族 P-130：写口不设档）。
    // 64 足够整表提交（全站字段数由站长自己控），又掐掉放大面。
    if body.values.len() > 64 {
        return Err(DomainError::Validation("一次最多提交 64 个字段".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let mut saved = 0i64;
    let mut skipped = 0i64;
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
            // 未知/停用/模块关闭的字段不收，但**要如实回报**（P-138）：
            // 旧写法只 `continue`，响应里 saved=0 与「全都不认识」长得一样，
            // 用户在 usercp 点了保存、看到成功，值其实一个都没落。
            skipped += 1;
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
            .map_err(|e| DomainError::Validation(named(&_label, &e)))?;
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
    Ok(ok(serde_json::json!({
        "saved": saved, "skipped": skipped,
    })))
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

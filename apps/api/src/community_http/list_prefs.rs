//! 用户级列表偏好（0316）：GET/POST /me/list-prefs。
//!
//! 站点级 view_hidden（E6 视图布局）是**全站统一**的；本模块给用户一层
//! **个人覆盖**：视图形态 / 每页条数 / 额外隐藏列。对标 Arcadia layout config。
//!
//! 存储沿用 users 的 JSONB 列范式（同 notice_prefs），缺省 '{}' = 全走站点默认。
//! 键白名单在写入端把关，防塞任意 JSON。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 允许的列键（与前端 TORRENT_VIEWS / 表格列一一对应；title/选择/行为列恒显，
/// 不允许用户隐藏，故不在白名单——见 torrents-table.tsx 的门控口径）。
const COL_KEYS: [&str; 8] = [
    "cat",
    "cover",
    "comments",
    "alive",
    "size",
    "seeders",
    "leechers",
    "completed",
];
const VIEW_KEYS: [&str; 3] = ["table", "card", "poster"];
const PER_PAGE: [i64; 3] = [20, 50, 100];

/// 我的列表偏好（缺省 = 空对象，前端回落站点默认）
#[get("/me/list-prefs")]
async fn list_prefs_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let prefs: serde_json::Value =
        sqlx::query_scalar("SELECT list_prefs FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(serde_json::json!({}));
    Ok(ok(prefs))
}

#[derive(Deserialize)]
struct ListPrefsSetReq {
    /// 'view' | 'per_page' | 'hidden_cols'
    key: String,
    /// view → 字符串；per_page → 数字；hidden_cols → 字符串数组
    value: serde_json::Value,
}

/// 设置单项列表偏好（白名单键 + 值域校验）
#[post("/me/list-prefs")]
async fn list_prefs_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ListPrefsSetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (json_path, value) = match body.key.as_str() {
        "view" => {
            let v = body.value.as_str().ok_or_else(|| {
                DomainError::Validation("view 需为字符串".into())
            })?;
            if !VIEW_KEYS.contains(&v) {
                return Err(DomainError::Validation(
                    "view 只能是 table/card/poster".into(),
                ));
            }
            ("view", serde_json::json!(v))
        }
        "per_page" => {
            let n = body.value.as_i64().ok_or_else(|| {
                DomainError::Validation("per_page 需为数字".into())
            })?;
            if !PER_PAGE.contains(&n) {
                return Err(DomainError::Validation(
                    "per_page 只能是 20/50/100".into(),
                ));
            }
            ("per_page", serde_json::json!(n))
        }
        "hidden_cols" => {
            let arr = body.value.as_array().ok_or_else(|| {
                DomainError::Validation("hidden_cols 需为数组".into())
            })?;
            // 逐项过白名单，剔除未知列（不报错，静默过滤——老前端多传不致命）
            let mut cols: Vec<String> = Vec::new();
            for v in arr {
                let Some(s) = v.as_str() else { continue };
                if COL_KEYS.contains(&s) && !cols.iter().any(|c| c == s) {
                    cols.push(s.to_string());
                }
            }
            ("hidden_cols", serde_json::json!(cols))
        }
        other => {
            return Err(DomainError::Validation(format!(
                "未知偏好键 {other}（可选：view/per_page/hidden_cols）"
            )));
        }
    };
    sqlx::query(
        "UPDATE users SET list_prefs = jsonb_set(list_prefs, \
         ARRAY[$2], $3::jsonb, true) WHERE id = $1",
    )
    .bind(auth.id)
    .bind(json_path)
    .bind(value.to_string())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "key": json_path, "value": value })))
}

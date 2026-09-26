use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
// ============ 用户管理（第五轮：参考站 /nexusphp user/users 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserListRow {
    id: i64,
    username: String,
    email: String,
    class_id: i32,
    class_name: Option<String>,
    uploaded: i64,
    downloaded: i64,
    status: i16,
    download_enabled: bool,
    suspended: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 五维筛选 + 排序 + 分页（ID/等级/状态/启用/下载权限/挂起 + 用户名/邮箱搜索）
#[derive(Deserialize)]
struct UserListQ {
    #[serde(default)]
    q: String,
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    class_id: Option<i32>,
    /// 0=全部 1=正常 2=禁言 3=封禁
    #[serde(default)]
    status: Option<i16>,
    /// yes/no/全部
    #[serde(default)]
    enabled: Option<String>,
    #[serde(default)]
    download: Option<String>,
    #[serde(default)]
    suspended: Option<String>,
    /// 自定义字段过滤（G4）：字段 key（值关键词为空 = 「填过这个字段的人」）
    #[serde(default)]
    ffield: Option<String>,
    /// 字段值关键词（子串匹配；标量/多选/布尔都按值的 JSON 文本比）
    #[serde(default)]
    fval: Option<String>,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_desc")]
    desc: bool,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}
fn default_sort() -> String {
    "id".into()
}
fn default_desc() -> bool {
    true
}
pub fn default_page() -> i64 {
    1
}
pub fn default_per_page() -> i64 {
    20
}

#[get("/admin/users")]
async fn user_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserListQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    // 白名单排序字段（参考站可排序列：Id/等级/上传量/下载量/添加时间）
    let order_col = match q.sort.as_str() {
        "id" => "u.id",
        "class" => "u.class_id",
        "uploaded" => "u.uploaded",
        "downloaded" => "u.downloaded",
        "created" => "u.created_at",
        _ => "u.id",
    };
    // 条件占位符：$1=搜索 $2=ID $3=等级（WHERE 里按出现顺序绑定，见下方 bind 次序）
    let mut where_parts: Vec<String> = Vec::new();
    if q.id.is_some() {
        where_parts.push("u.id = $2".into());
    }
    if q.class_id.is_some() {
        where_parts.push("u.class_id = $3".into());
    }
    if let Some(st) = q.status {
        match st {
            0 => {}
            1 => where_parts.push("u.status = 0".into()),
            2 => where_parts.push("u.status = 1".into()),
            3 => where_parts.push("u.status >= 2".into()),
            _ => return Err(DomainError::Validation("status 取值 0-3".into())),
        }
    }
    if let Some(e) = q.enabled.as_deref() {
        match e {
            "yes" => where_parts.push("u.status < 2".into()),
            "no" => where_parts.push("u.status >= 2".into()),
            _ => {}
        }
    }
    if let Some(d) = q.download.as_deref() {
        match d {
            "yes" => where_parts.push("u.download_enabled".into()),
            "no" => where_parts.push("NOT u.download_enabled".into()),
            _ => {}
        }
    }
    if let Some(s) = q.suspended.as_deref() {
        match s {
            "yes" => where_parts.push("u.suspended".into()),
            "no" => where_parts.push("NOT u.suspended".into()),
            _ => {}
        }
    }
    if !q.q.trim().is_empty() {
        where_parts.push("(u.username ILIKE $1 OR u.email ILIKE $1)".into());
    }
    // 自定义字段过滤（G4）：$4=字段 key，$5=值关键词（NULL = 只要填过）
    let fk: Option<&str> =
        q.ffield.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if fk.is_some() {
        where_parts.push(
            "EXISTS (SELECT 1 FROM user_field_values fv \
             WHERE fv.user_id = u.id AND fv.field_key = $4 \
               AND ($5::text IS NULL OR fv.value::text ILIKE $5))"
                .into(),
        );
    }
    let where_sql = if where_parts.is_empty() {
        "TRUE".to_string()
    } else {
        where_parts.join(" AND ")
    };
    let dir = if q.desc { "DESC" } else { "ASC" };
    let sql = format!(
        r#"SELECT u.id, u.username, u.email, u.class_id, c.name AS class_name,
                  u.uploaded, u.downloaded, u.status, u.download_enabled, u.suspended,
                  u.created_at, u.last_seen_at
           FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
           WHERE {where_sql}
           ORDER BY {order_col} {dir}, u.id {dir}
           LIMIT $6 OFFSET $7"#,
    );
    let pattern = crate::http::like_pattern(&q.q);
    let fval_pat = q
        .fval
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(crate::http::like_pattern);
    let rows: Vec<AdminUserListRow> = sqlx::query_as(&sql)
        .bind(pattern.clone())
        .bind(q.id)
        .bind(q.class_id)
        .bind(fk)
        .bind(fval_pat.clone())
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let count_sql = format!("SELECT count(*) FROM users u WHERE {where_sql}");
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(pattern)
        .bind(q.id)
        .bind(q.class_id)
        .bind(fk)
        .bind(fval_pat)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows,
        "total": total,
        "page": q.page.max(1),
        "per_page": q.per_page,
    })))
}

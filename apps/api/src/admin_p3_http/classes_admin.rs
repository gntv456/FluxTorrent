//! 等级体系后台化（一审 R4.2 / 二审 G4-4）：class_rules + user_classes 的
//! 管理 CRUD。此前只有只读 GET /classes，调档位只能走 SQL 迁移（0172 即教训）。
//! staff 档（class_id >= 90）锁死不可改——安全边界；升降级规则由
//! worker class_auto_adjust 消费（demotable/promo_sparks 同步生效）。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// staff 档保护线：90+ 是维护开发员/主管/sysop 档（0172 口径），不开放后台编辑
const STAFF_CLASS_FLOOR: i32 = 90;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminClassRow {
    class_id: i32,
    name: String,
    min_uploaded: i64,
    min_download_count: i32,
    min_seed_hours: i32,
    min_account_age_days: i32,
    demotable: bool,
    promo_sparks: i64,
    /// 当前该档用户数（改阈值前的心里有数）
    users: i64,
}

/// GET /admin/classes：全量等级规则（含 staff 档只读展示）
#[get("/admin/classes")]
pub async fn admin_classes_list(
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
    let rows: Vec<AdminClassRow> = sqlx::query_as(
        "SELECT cr.class_id, cr.name, cr.min_uploaded, cr.min_download_count, \
             cr.min_seed_hours, cr.min_account_age_days, cr.demotable, \
             cr.promo_sparks, \
             (SELECT count(*) FROM users u WHERE u.class_id = cr.class_id) \
             AS users \
         FROM class_rules cr ORDER BY cr.class_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ClassPutBody {
    name: String,
    min_uploaded: i64,
    min_download_count: i32,
    min_seed_hours: i32,
    min_account_age_days: i32,
    #[serde(default)]
    demotable: bool,
    #[serde(default)]
    promo_sparks: i64,
}

/// PUT /admin/classes/{id}：改单档规则（1..=12；staff 档 403）
#[put("/admin/classes/{id}")]
pub async fn admin_class_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<ClassPutBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    if id >= STAFF_CLASS_FLOOR || id < 1 {
        return Err(DomainError::Validation(
            "仅开放 1-12 用户档编辑（staff 档位不开放后台修改）".into(),
        ));
    }
    if body.name.trim().is_empty() || body.name.len() > 32 {
        return Err(DomainError::Validation("档位名需 1-32 字符".into()));
    }
    if body.min_uploaded < 0 || body.min_download_count < 0
        || body.min_seed_hours < 0 || body.min_account_age_days < 0
        || body.promo_sparks < 0
    {
        return Err(DomainError::Validation("阈值/奖励不能为负".into()));
    }
    // 同步两张表：class_rules（升降级规则）与 user_classes（档案展示名）。
    // user_classes.id 有 users 外键，只 UPDATE 不 DELETE/INSERT（档位数固定）。
    let n = sqlx::query(
        "UPDATE class_rules SET name = $2, min_uploaded = $3, \
             min_download_count = $4, min_seed_hours = $5, \
             min_account_age_days = $6, demotable = $7, promo_sparks = $8 \
         WHERE class_id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.min_uploaded)
    .bind(body.min_download_count)
    .bind(body.min_seed_hours)
    .bind(body.min_account_age_days)
    .bind(body.demotable)
    .bind(body.promo_sparks)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("档位不存在".into()));
    }
    sqlx::query("UPDATE user_classes SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(body.name.trim())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "admin_class_put", None)
        .await;
    Ok(ok(serde_json::json!({ "updated": id })))
}

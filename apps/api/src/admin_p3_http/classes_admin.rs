//! 等级体系后台化（一审 R4.2 / 二审 G4-4）：class_rules + user_classes 的
//! 管理 CRUD。此前只有只读 GET /classes，调档位只能走 SQL 迁移（0172 即教训）。
//! staff 档（class_id >= 90）锁死不可改——安全边界；升降级规则由
//! worker class_auto_adjust 消费（demotable/promo_sparks 同步生效）。
//!
//! 档位全集以 `user_classes` 为准（批十七 P-23）：`class_rules` 只有
//! 0-12 成长档，贵宾（20）与职务档（91+）只存在于 user_classes。此前
//! GET 直接读 class_rules，把这 8 档连同「staff 档只读展示」的承诺一起
//! 变没了；档位名也取的是 class_rules 别名（LV1/LV2…），而成员档案与
//! 公开等级页显示的是 user_classes.name（User/Power User…）——站长在
//! 后台改的名字和成员看到的名字是两列。现按 user_classes 出全集，
//! 名称取展示名，无规则行的档位在界面上只读（PUT 一并拒绝）。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// staff 档保护线：90+ 是维护开发员/主管/sysop 档（0172 口径），不开放后台编辑
pub(crate) const STAFF_CLASS_FLOOR: i32 = 90;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminClassRow {
    class_id: i32,
    /// 展示名（user_classes.name，与成员档案/公开等级页同源）
    name: String,
    /// 以下六列取自 class_rules；None = 该档无升降级规则（贵宾/职务档），
    /// 面板按只读渲染、PUT 拒绝
    min_uploaded: Option<i64>,
    min_download_count: Option<i32>,
    min_seed_hours: Option<i32>,
    min_account_age_days: Option<i32>,
    demotable: Option<bool>,
    promo_sparks: Option<i64>,
    /// 当前该档用户数（改阈值前的心里有数）
    users: i64,
}

/// GET /admin/classes：档位全集（含无升降级规则的贵宾/职务档，只读）
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
        "SELECT uc.id AS class_id, uc.name, cr.min_uploaded, \
             cr.min_download_count, cr.min_seed_hours, \
             cr.min_account_age_days, cr.demotable, cr.promo_sparks, \
             (SELECT count(*) FROM users u WHERE u.class_id = uc.id) \
             AS users \
         FROM user_classes uc \
             LEFT JOIN class_rules cr ON cr.class_id = uc.id \
         ORDER BY uc.id",
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

/// PUT /admin/classes/{id}：改单档规则（1..=12；staff 档与无规则档拒绝）
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
    // GET 现在按 user_classes 出全集，贵宾（20）等档位在 class_rules 里
    // 没有规则行——它们出现在面板上是为了可读，不是可编辑。
    let has_rule: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM class_rules WHERE class_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !has_rule {
        return Err(DomainError::Validation(
            "该档位没有升降级规则（贵宾/职务档不开放后台编辑）".into(),
        ));
    }
    if body.name.trim().is_empty() || body.name.trim().chars().count() > 32 {
        return Err(DomainError::Validation("档位名需 1-32 字符".into()));
    }
    if body.min_uploaded < 0
        || body.min_download_count < 0
        || body.min_seed_hours < 0
        || body.min_account_age_days < 0
        || body.promo_sparks < 0
    {
        return Err(DomainError::Validation("阈值/奖励不能为负".into()));
    }
    // 同步两张表：class_rules（升降级规则）与 user_classes（档案展示名）。
    // user_classes.id 有 users 外键，只 UPDATE 不 DELETE/INSERT（档位数固定）。
    // 两张表必须同事务：旧写法逐条自动提交，class_rules 改完之后
    // user_classes 那条失败就留下「规则表新名 / 档案表旧名」的分裂
    //（面板读一张、成员侧读另一张）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("档位不存在".into()));
    }
    let n2 = sqlx::query(
        "UPDATE user_classes SET name = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n2 == 0 {
        return Err(DomainError::Validation(
            "档位档案行缺失（user_classes），已回滚".into(),
        ));
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(Some(auth.id), "admin_class_put", Some(id as i64),
            None, Some(serde_json::json!({"name": body.name.trim()})))
        .await;
    Ok(ok(serde_json::json!({ "updated": id })))
}

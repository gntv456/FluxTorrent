//! P2-7 勋章 CRUD（主体）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-7 勋章 CRUD 与持有管理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalAdminRow {
    id: i64,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    rarity: Option<String>,
    limited: bool,
    get_type: i16,
    duration_days: Option<i32>,
    bonus_addition_factor: Option<f64>,
    category_id: i32,
    asset_ref: Option<String>,
    held_count: i64,
}

#[get("/admin/medals")]
async fn admin_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MedalAdminRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.description, m.price, m.rarity, m.limited, m.get_type,
                  m.duration_days, m.bonus_addition_factor::float8, m.category_id, m.asset_ref,
                  (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id)::bigint AS held_count
           FROM medals m ORDER BY m.category_id, m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct MedalReq {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    price: Option<i64>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    limited: Option<bool>,
    #[serde(default)]
    get_type: Option<i16>,
    #[serde(default)]
    duration_days: Option<i32>,
    #[serde(default)]
    bonus_addition_factor: Option<f64>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    asset_ref: Option<String>,
}

#[post("/admin/medals")]
async fn admin_medal_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("勋章名不能为空".into()));
    }
    // 审计修复（P1）：此前负价格/负时长直接穿透到 DB CHECK，报 500 内部错误。
    if let Some(pr) = body.price {
        if pr < 0 {
            return Err(DomainError::Validation("价格不能为负".into()));
        }
    }
    if let Some(d) = body.duration_days {
        if d < 0 {
            return Err(DomainError::Validation("有效天数不能为负".into()));
        }
    }
    if let Some(f) = body.bonus_addition_factor {
        if !(0.0..=10.0).contains(&f) {
            return Err(DomainError::Validation(
                "加成系数需在 0-10 之间".into(),
            ));
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO medals (name, description, price, rarity, limited, get_type, duration_days, bonus_addition_factor, category_id, asset_ref) \
         VALUES ($1, $2, $3, $4, COALESCE($5, FALSE), COALESCE($6, 2), $7, $8::numeric, COALESCE($9, 0), $10) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.description.clone())
    .bind(body.price)
    .bind(body.rarity.clone())
    .bind(body.limited)
    .bind(body.get_type)
    .bind(body.duration_days)
    .bind(body.bonus_addition_factor)
    .bind(body.category_id)
    .bind(body.asset_ref.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "medal.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/medals/{id}")]
async fn admin_medal_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MedalReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE medals SET name = $2, description = $3, price = $4, rarity = $5, limited = COALESCE($6, limited), \
           get_type = COALESCE($7, get_type), duration_days = $8, bonus_addition_factor = COALESCE($9::numeric, bonus_addition_factor), \
           category_id = COALESCE($10, category_id), asset_ref = $11 WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.description.clone())
    .bind(body.price)
    .bind(body.rarity.clone())
    .bind(body.limited)
    .bind(body.get_type)
    .bind(body.duration_days)
    .bind(body.bonus_addition_factor)
    .bind(body.category_id)
    .bind(body.asset_ref.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "medal.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/medals/{id}")]
async fn admin_medal_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM medals WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "medal.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

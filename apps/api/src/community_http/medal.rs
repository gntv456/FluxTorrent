//! M14 勋章。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ M14 勋章 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRow {
    id: i64,
    name: String,
    price: Option<i64>,
    rarity: Option<String>,
    limited: bool,
    owned: bool,
    wearing: bool,
    description: Option<String>,
    duration_days: Option<i32>,
    get_type: i16,
    sale_begin_at: Option<chrono::DateTime<chrono::Utc>>,
    sale_end_at: Option<chrono::DateTime<chrono::Utc>>,
    inventory: Option<i32>,
    bonus_addition_factor: f64,
    category_id: i32,
    category_name: Option<String>,
    /// 勋章图片（asset_ref，0001 就有列；此前只在后台接口返回，前台拿不到 → 全站只能画 🏅）
    asset_ref: Option<String>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRarityRow {
    value: String,
    label: String,
    /// 配色档（前端映射到 tailwind token 类）
    tone: String,
    sort: i32,
}

/// 勋章稀有度词表（0143）：前台角标与后台下拉共用；词表主体在后台维护。
/// 免鉴权——纯展示元数据，且 /medals 页面本身就可能以未登录态渲染。
#[get("/medal-rarities")]
async fn medal_rarities(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<MedalRarityRow> = sqlx::query_as(
        "SELECT value, label, tone, \
         sort FROM medal_rarities ORDER BY sort, value",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[get("/medals")]
async fn medal_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await.ok();
    let uid = auth.map(|a| a.id);
    let rows = sqlx::query_as::<_, MedalRow>(
                "SELECT m.id, m.name, m.price, m.rarity, m.limited, \
         m.description, m.duration_days, m.get_type, m.sale_begin_at, \
         m.sale_end_at, m.inventory, m.bonus_addition_factor::float8, \
         m.category_id, c.name AS category_name, m.asset_ref, \
         ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()))) AS owned, \
         ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()))) AS wearing FROM medals m LEFT JOIN medal_categories c ON c.id = m.category_id ORDER BY m.category_id, \
         m.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WearReq {
    medal_id: Option<i64>,
}

#[put("/medals/wear")]
async fn medal_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 单佩戴位：先全部摘下再佩戴指定勋章。两条语句需原子完成——否则「佩戴未拥有勋章」
    // 报错回滚时会把原本已佩戴的勋章也摘掉（先摘后戴的非事务写已生效，无法随错误回退）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE user_medals SET wearing = false WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(mid) = body.medal_id {
        let updated = sqlx::query(
            "UPDATE user_medals SET wearing = true WHERE user_id = \
             $1 AND medal_id = $2",
        )
        .bind(auth.id)
        .bind(mid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if updated.rows_affected() == 0 {
            // 回滚：保留佩戴原状（未拥有该勋章时不动已佩戴勋章）
            tx.rollback()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            return Err(DomainError::Validation("未拥有该勋章".into()));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "wearing": body.medal_id })))
}

#[get("/me/medals")]
async fn my_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, String, bool)> = sqlx::query_as(
                "SELECT m.id, m.name, \
         um.wearing FROM user_medals um JOIN medals m ON m.id = um.medal_id WHERE um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()) ORDER BY m.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

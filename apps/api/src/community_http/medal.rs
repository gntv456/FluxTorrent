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
    /// per-勋章赠送手续费（基点；NULL = 回退全站 gift_tax_bp，0204）
    gift_fee_bp: Option<i32>,
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

/// 佩戴上限（0204）：site_settings.medals_max_worn，缺省 3，clamp 1..=12。
/// 各展示位（/me、me_settings）的 worn_medals LIMIT 同源此键。
pub async fn medals_max_worn(db: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'medals_max_worn'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v: String| v.parse().ok())
    .map(|v: i64| v.clamp(1, 12))
    .unwrap_or(3)
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
         m.category_id, c.name AS category_name, m.asset_ref, m.gift_fee_bp, \
         ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()))) AS owned, \
         ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()))) AS wearing FROM medals m LEFT JOIN medal_categories c ON c.id = m.category_id ORDER BY m.category_id, \
         m.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let max_worn = medals_max_worn(&state.repo.db).await;
    Ok(ok(
        serde_json::json!({ "items": rows, "max_worn": max_worn }),
    ))
}

#[derive(Deserialize)]
struct WearReq {
    medal_id: Option<i64>,
    /// 0204 多佩戴：true=戴上（校验上限），false=摘下。
    /// 缺省（旧客户端）按 toggle 处理：未戴→戴（占一个上限位），已戴→摘。
    #[serde(default)]
    wear: Option<bool>,
}

#[put("/medals/wear")]
async fn medal_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 0204 多佩戴：摘掉全部仅剩「显式清空」语义（medal_id = null）；
    // 单枚戴/摘不再先清场。上限内自由多戴，超限报 Validation。
    let Some(mid) = body.medal_id else {
        sqlx::query(
            "UPDATE user_medals SET wearing = false WHERE user_id = $1",
        )
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(ok(serde_json::json!({ "cleared": true })));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 锁行读当前佩戴状态，决定目标 wear 值（toggle 兼容）
    let currently: Option<bool> = sqlx::query_scalar(
        "SELECT um.wearing FROM user_medals um \
         WHERE um.user_id = $1 AND um.medal_id = $2 \
         AND (um.expires_at IS NULL OR um.expires_at > now()) \
         FOR UPDATE",
    )
    .bind(auth.id)
    .bind(mid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(currently) = currently else {
        tx.rollback()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Err(DomainError::Validation("未拥有该勋章".into()));
    };
    let wear = body.wear.unwrap_or(!currently);
    if wear && !currently {
        let max_worn = medals_max_worn(&state.repo.db).await;
        let worn: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM user_medals WHERE user_id = $1 AND wearing",
        )
        .bind(auth.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if worn >= max_worn {
            tx.rollback()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            return Err(DomainError::Validation(format!(
                "佩戴已达上限（最多 {max_worn} 枚）"
            )));
        }
    }
    if wear != currently {
        sqlx::query(
            "UPDATE user_medals SET wearing = $3 WHERE user_id = $1 AND medal_id = $2",
        )
        .bind(auth.id)
        .bind(mid)
        .bind(wear)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "medal_id": mid, "wearing": wear })))
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
    let max_worn = medals_max_worn(&state.repo.db).await;
    Ok(ok(
        serde_json::json!({ "items": rows, "max_worn": max_worn }),
    ))
}

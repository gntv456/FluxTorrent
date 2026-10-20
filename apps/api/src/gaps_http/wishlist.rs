//! 心愿单（wishlist）端点。
//! 从 gaps_http/misc.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct WishRow {
    id: i64,
    keyword: String,
    category_id: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct WishHitRow {
    torrent_id: i64,
    torrent_name: String,
    keyword: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/wishlist")]
pub async fn wishlist_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<WishRow> = sqlx::query_as(
                "SELECT id, keyword, category_id, \
         created_at FROM wishlist WHERE user_id = $1 ORDER BY id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 命中历史（0284 P2-10）：最近 20 条，「我的 → 愿望单」可回看，
    // 不再只靠一条站内信。注意响应从裸数组升级为 {items, hits}——
    // 消费方 wishlist-panel.tsx 同步升级
    let hits: Vec<WishHitRow> = sqlx::query_as(
        "SELECT h.torrent_id, t.name AS torrent_name, h.keyword, \
         h.created_at FROM wishlist_hits h \
         JOIN torrents t ON t.id = h.torrent_id \
         WHERE h.user_id = $1 ORDER BY h.id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "items": rows, "hits": hits })))
}

/// P-314 同族（写口不拒未知键）：`category_id` 拼成 `cat_id`/`categoryId` 时，
/// 旧写法照样 200 建出一条**不限分类**的愿望，而客户端以为限了分类——
/// 之后每次命中都比预期宽，会员只看到「莫名其妙推我一条」。
/// 前端实际载荷只有 `keyword`（wishlist.tsx:66/134、
/// torrent-actionbar-mount.tsx:62），加这条对现有界面零行为变化。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WishAddReq {
    keyword: String,
    #[serde(default)]
    category_id: Option<i32>,
}

#[post("/wishlist")]
pub async fn wishlist_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishAddReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = body.keyword.trim();
    // 按**字**判（P-128 同族，批三十八先例）：旧写法 .len() 数的是 UTF-8 字节，
    // 而文案写「1-100」，中文用户实际只配 33 字；torrent-actionbar-mount.tsx:67
    // 是拿**种名**当关键词提交的，中文长名一点就 400。
    if kw.is_empty() || kw.chars().count() > 100 {
        return Err(DomainError::Validation("关键词长度 1-100".into()));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM wishlist WHERE user_id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if count >= 20 {
        return Err(DomainError::Validation(
            "愿望单上限 20 条，请先删除旧的".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO wishlist (user_id, keyword, category_id) \
         VALUES ($1, $2, $3) ON CONFLICT (user_id, keyword) DO \
         UPDATE SET category_id = EXCLUDED.category_id",
    )
    .bind(auth.id)
    .bind(kw)
    .bind(body.category_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| crate::errors::db_to_domain(e, "愿望单的分类"))?;
    Ok(ok(serde_json::json!({ "keyword": kw })))
}

/// 删除口同理：`{id}` 拼成 `{wish_id}` 时会因缺字段报错，但反过来多带一个键
/// （如从别的删除口抄来的 `ids`）旧写法静默忽略、照样回 200 removed。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WishDelReq {
    id: i64,
}

#[post("/wishlist/remove")]
pub async fn wishlist_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishDelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query("DELETE FROM wishlist WHERE id = $1 AND user_id = $2")
        .bind(body.id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    Ok(ok(serde_json::json!({ "removed": body.id })))
}

//! 厂牌订阅（0332 documentary 专项收官）：订某出品方的新片。
//!
//! 与 `publish_http/group.rs` 的组订阅同范式（0030/0075），差别是订阅
//! 目标为 `content_networks` 实体（0330）。写路径只做「登记订阅」，
//! 发信在过审副作用里（`admin_http/review_side_effects.rs`）——
//! 与组订阅一样，本模块不产生通知。
//!
//! 端点数（docs 口径：一个资源一条链，订阅/退订/我的订阅三件）：
//!   · `POST   /networks/{id}/subscribe`
//!   · `POST   /networks/{id}/unsubscribe`
//!   · `GET    /me/subscriptions/networks`

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct SubQuery {
    #[serde(default)]
    kind: Option<String>,
}

/// 订阅厂牌（幂等：重复订阅不报错，与组订阅 ON CONFLICT DO NOTHING 同口径）
#[post("/networks/{id}/subscribe")]
pub async fn network_subscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let nid = path.into_inner();
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM content_networks WHERE id = $1)",
    )
    .bind(nid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(nid));
    }
    sqlx::query(
        "INSERT INTO network_subscriptions (user_id, network_id) \
         VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(auth.id)
    .bind(nid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": nid })))
}

/// 退订厂牌
#[post("/networks/{id}/unsubscribe")]
pub async fn network_unsubscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let nid = path.into_inner();
    sqlx::query(
        "DELETE FROM network_subscriptions \
         WHERE user_id = $1 AND network_id = $2",
    )
    .bind(auth.id)
    .bind(nid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "unsubscribed": nid })))
}

/// 我的厂牌订阅（带该厂牌在档过审种数，对齐 /artists 列表口径）
#[get("/me/subscriptions/networks")]
pub async fn my_network_subscriptions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SubQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let kind = q.kind.as_deref().unwrap_or("network");
    let rows: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT cn.id, cn.name, cn.kind, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
            JOIN section_dict sd ON sd.id = ts.dict_id \
            JOIN torrents t ON t.id = ts.torrent_id \
           WHERE ts.kind = cn.kind AND sd.name = cn.name \
             AND t.approval_status = 1) \
         FROM network_subscriptions ns \
         JOIN content_networks cn ON cn.id = ns.network_id \
         WHERE ns.user_id = $1 AND cn.kind = $2 \
         ORDER BY cn.norm_name",
    )
    .bind(auth.id)
    .bind(kind)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows
        .iter()
        .map(|(id, name, k, n)| {
            serde_json::json!({
                "network_id": id, "name": name,
                "kind": k, "torrents": n,
            })
        })
        .collect::<Vec<_>>()))
}

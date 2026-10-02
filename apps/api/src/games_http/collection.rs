//! 通用「图鉴/收集」读端点（样图 2026-10 补页批）：按游戏读奖池档位 ×
//! 用户命中次数。钓鱼已有一份（fishing_extra::fishing_collection，含
//! 事件鱼双池合并）；这里给扭蛋/九宫格/刮刮乐/转盘复用同一口径，避免
//! 四处各写一份 join。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::pool::{dberr, load_pool};

type Db = web::Data<std::sync::Arc<AppState>>;

/// 支持图鉴的游戏（与 arcade_pool_rounds 落表的游戏一致）
const GAMES: [&str; 4] = ["capsule", "jgg", "scratch", "wheel"];

#[get("/games/collection/{game}")]
pub(super) async fn game_collection(
    req: HttpRequest,
    state: Db,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let game = path.into_inner();
    if !GAMES.contains(&game.as_str()) {
        return Err(DomainError::Validation(format!(
            "「{game}」没有图鉴（可选 {}）",
            GAMES.join("/")
        )));
    }
    let db = &state.repo.db;
    let caught: Vec<(String, i64)> = sqlx::query_as(
        "SELECT prize, count(*)::bigint FROM arcade_pool_rounds \
          WHERE user_id = $1 AND game = $2 GROUP BY prize",
    )
    .bind(auth.id)
    .bind(&game)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let pool = load_pool(db, &game).await?;
    let ticket = pool.ticket;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for (i, e) in pool.entries.iter().enumerate() {
        let (rar, img) = pool.meta.get(i).cloned().unwrap_or((1, String::new()));
        let count = caught
            .iter()
            .find(|(p, _)| p == &e.label)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        entries.push(serde_json::json!({
            "label": e.label, "rarity": rar, "image": img,
            "count": count,
            "weight": e.weight,
            "mult": e.mult_permille() as f64 / 1000.0,
            "value": e.value(ticket),
        }));
    }
    let total = entries.len() as i64;
    let got = entries
        .iter()
        .filter(|f| f["count"].as_i64().unwrap_or(0) > 0)
        .count() as i64;
    Ok(ok(serde_json::json!({
        "game": game, "got": got, "total": total, "entries": entries,
    })))
}

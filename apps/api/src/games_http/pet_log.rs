use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

/// 宠物互动记录（样图⑨「互动记录」子页）：今日统计（喂食次数 / 领取
/// 产出 / 摸摸头）+ 近期互动列表。
///
/// 数据源是 audit_log（game.pet.feed / claim / customize 都留痕）——
/// 宠物没有独立的对局表；这里只读聚合，不新造写路径。
#[get("/games/pet/log")]
pub(super) async fn pet_log(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT action, created_at FROM audit_log \
          WHERE actor_id = $1 AND action LIKE 'game.pet.%' \
          ORDER BY id DESC LIMIT 30",
    )
    .bind(auth.id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let today: Vec<(String, i64)> = sqlx::query_as(
        "SELECT action, count(*)::bigint FROM audit_log \
          WHERE actor_id = $1 AND action LIKE 'game.pet.%' \
            AND created_at >= date_trunc('day', now()) \
          GROUP BY 1",
    )
    .bind(auth.id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let count = |key: &str| {
        today
            .iter()
            .find(|(a, _)| a == key)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    };
    Ok(ok(serde_json::json!({
        "today": {
            "feed": count("game.pet.feed"),
            "claim": count("game.pet.claim"),
            "customize": count("game.pet.customize"),
        },
        "items": rows
            .iter()
            .map(|(action, at)| {
                serde_json::json!({
                    "action": action.trim_start_matches("game.pet."),
                    "at": at,
                })
            })
            .collect::<Vec<_>>(),
    })))
}

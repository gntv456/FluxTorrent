//! 我的对局战绩（一局一条）。自 overview.rs 拆出守 300 行门禁；
//! SQL 下沉 repo/game_rounds.rs（分层宪法：H 层零 SQL）。streak 是
//! 波3 留存层：从最近一局向前数连续 net>0 的局数（此前四页 StreakPill
//! 是 useState——刷新即丢；服务端按流水算，刷新/换设备口径一致）。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::game_rounds as rounds_repo;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};

use super::overview::HistoryQuery;

#[get("/games/rounds")]
pub(super) async fn game_rounds(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<HistoryQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = rounds_repo::game_rounds(
        &state.repo.db,
        auth.id,
        q.game.as_deref(),
        q.limit.unwrap_or(20).clamp(1, 50),
    )
    .await?;
    // rows 按 id DESC 有序，线性扫即可。
    let mut streak = 0i64;
    for r in &rows {
        if r.net > 0 {
            streak += 1;
        } else {
            break;
        }
    }
    Ok(ok(serde_json::json!({ "rows": rows, "streak": streak })))
}

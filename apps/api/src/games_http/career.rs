//! 游戏生涯聚合（波3 留存层）：从 overview.rs 拆出（行数门禁）。
//! 与 rounds/history 同一本账（spark_ledger kind='game'），只读。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 我的游戏生涯（波3 留存层）：总场次/总下注/最大单赢/各玩法分布。
/// 全从 spark_ledger(kind='game') 聚合，不建新表；写入侧零改动。
#[get("/games/career")]
pub(super) async fn game_career(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 按 ref_type 聚合：局数 = 下注行数，下注合计 = -Σbet，
    // 派彩走幂等键 join 与 rounds 同一把尺
    let per_game: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT l.ref_type, count(*)::bigint, \
                -sum(l.amount)::bigint, \
                COALESCE(sum(COALESCE(w.amount, 0)), 0)::bigint \
         FROM spark_ledger l \
         LEFT JOIN spark_ledger w \
           ON w.idempotency_key IN ( \
                'game-' || l.ref_type || '-win:' || l.idempotency_key, \
                'game-bs-win:' || l.idempotency_key \
              ) \
         WHERE l.user_id = $1 AND l.kind = 'game' AND l.amount < 0 \
           AND l.ref_type IS NOT NULL \
         GROUP BY l.ref_type ORDER BY count(*) DESC",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let plays: i64 = per_game.iter().map(|(_, n, _, _)| n).sum();
    let wagered: i64 = per_game.iter().map(|(_, _, b, _)| b).sum();
    let returned: i64 = per_game.iter().map(|(_, _, _, p)| p).sum();
    let best: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'game' AND amount > 0",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "plays": plays,
        "wagered": wagered,
        "returned": returned,
        "net": returned - wagered,
        "best_win": best,
        "games": per_game
            .iter()
            .map(|(g, n, b, p)| serde_json::json!({
                "game": g, "plays": n, "wagered": b, "returned": p,
            }))
            .collect::<Vec<_>>(),
    })))
}

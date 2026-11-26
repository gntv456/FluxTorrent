//! 斗地主段位（只读派生）：从 settled 局聚合胜/负/场次/净分，按胜场
//! 阶梯派生段位档与档内星级。不建持久化段位表、不增发经济——刷新即一致，
//! 后续赛季星数沉淀再另建表。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::games::doudizhu_repo::rank_summary;
use crate::http::require_auth;
use crate::state::AppState;

/// 段位阶梯：(下限胜场, 档 key)。档内星级按到下一档的进度换算 1..=3。
const TIERS: [(i64, &str); 7] = [
    (0, "rookie"),
    (1, "beginner"),
    (5, "junior"),
    (15, "intermediate"),
    (30, "senior"),
    (60, "master"),
    (100, "god"),
];

#[derive(Serialize)]
struct RankView {
    tier: usize,
    name_key: String,
    stars: i64,
    /// 档内进度 0..=1（最高档恒 1）。
    progress: f64,
    wins: i64,
}

fn derive_rank(wins: i64) -> RankView {
    let mut tier = 0usize;
    for (i, (min, _)) in TIERS.iter().enumerate() {
        if wins >= *min {
            tier = i;
        }
    }
    let (cur_min, _) = TIERS[tier];
    let (next_min, _) = TIERS
        .get(tier + 1)
        .copied()
        .unwrap_or((cur_min, TIERS[tier].1));
    let progress = if next_min <= cur_min {
        1.0
    } else {
        ((wins - cur_min) as f64 / (next_min - cur_min) as f64).clamp(0.0, 1.0)
    };
    let stars = if tier == TIERS.len() - 1 {
        3
    } else {
        1 + (progress * 3.0).floor() as i64
    }
    .clamp(1, 3);
    RankView {
        tier,
        name_key: TIERS[tier].1.to_string(),
        stars,
        progress,
        wins,
    }
}

#[get("/games/doudizhu/rank")]
pub(super) async fn dz_rank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sum = rank_summary(&state.repo.db, auth.id).await?;
    let decided = sum.wins + sum.losses;
    let win_rate = if decided > 0 {
        sum.wins as f64 / decided as f64
    } else {
        0.0
    };
    let rank = derive_rank(sum.wins);
    Ok(ok(serde_json::json!({
        "plays": sum.plays,
        "wins": sum.wins,
        "losses": sum.losses,
        "net": sum.net,
        "win_rate": (win_rate * 100.0).round() / 100.0,
        "rank": rank,
    })))
}

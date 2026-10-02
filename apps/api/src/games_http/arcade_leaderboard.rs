//! 娱乐屋周榜（只读）：欧皇 / 渔王 / 农夫 / 劳模四张榜。
//!
//! 只读不派奖 —— 先让榜「看得见」；派奖属确定侧发放，将来接
//! `arcade_budget_*` 预算闸另批，不能绕过它直接从榜上发。
//! 周界与行为联动同口径（UTC 周 + 8h，linkage.rs 同式）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::pool::dberr;

const WEEK: &str = "l.created_at >= date_trunc('week', \
                    now() AT TIME ZONE 'UTC') + interval '8 hours'";

/// 跑一张榜：内层按用户聚合，外层连用户名、剔除封禁、取前 10。
async fn board(
    db: &PgPool,
    inner: &str,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT u.username, x.v::bigint FROM ( {inner} ) x \
           JOIN users u ON u.id = x.user_id \
          WHERE u.status < 2 ORDER BY x.v DESC LIMIT 10"
    ))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(who, v)| serde_json::json!({ "who": who, "v": v }))
        .collect())
}

#[get("/games/arcade/leaderboard")]
pub(super) async fn arcade_leaderboard(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 欧皇：本周单笔最大派彩。只认**对局派彩**（排除宠物领取/退款/补偿类
    // 的 kind='game' 入账——它们不是赌局结果，混进来会靠喂猫登顶）
    let lucky = board(
        db,
        &format!(
            "SELECT l.user_id, max(l.amount) AS v FROM spark_ledger l \
              WHERE l.kind = 'game' AND l.amount > 0 \
                AND (l.ref_type IN ('scratch','bigsmall','jgg','capsule',\
                     'wheel','fishing','farm') \
                     OR l.idempotency_key LIKE 'game-fishing-win:%') \
                AND {WEEK} \
              GROUP BY l.user_id"
        ),
    )
    .await
    .map_err(dberr)?;
    // 渔王：本周单竿最大渔获（派彩幂等键带 fishing-win 前缀）
    let fishing = board(
        db,
        &format!(
            "SELECT l.user_id, max(l.amount) AS v FROM spark_ledger l \
              WHERE l.kind = 'game' AND l.amount > 0 \
                AND l.idempotency_key LIKE 'game-fishing-win:%' AND {WEEK} \
              GROUP BY l.user_id"
        ),
    )
    .await
    .map_err(dberr)?;
    // 农夫：本周播种次数
    let farmer = board(
        db,
        &format!(
            "SELECT l.user_id, count(*) AS v FROM spark_ledger l \
              WHERE l.kind = 'game' AND l.amount < 0 \
                AND l.ref_type = 'farm_plant' AND {WEEK} GROUP BY l.user_id"
        ),
    )
    .await
    .map_err(dberr)?;
    // 劳模：本周总局数
    let grinder = board(
        db,
        &format!(
            "SELECT l.user_id, count(*) AS v FROM spark_ledger l \
              WHERE l.kind = 'game' AND l.amount < 0 AND {WEEK} \
              GROUP BY l.user_id"
        ),
    )
    .await
    .map_err(dberr)?;
    let week: String = sqlx::query_scalar(
        "SELECT to_char(date_trunc('week', now() AT TIME ZONE 'UTC') \
              + interval '8 hours', 'IYYY\"W\"IW')",
    )
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(ok(serde_json::json!({
        "week": week,
        "boards": {
            "lucky": lucky, "fishing": fishing,
            "farmer": farmer, "grinder": grinder,
        },
    })))
}

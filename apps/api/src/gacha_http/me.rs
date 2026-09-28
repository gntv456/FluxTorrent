//! GET /gacha/me：登录用户的个人面——双余额、图鉴/持有、最近抽取。
//! 图鉴口径（方案 §2 判据）：「已获得」与持有数解耦——lit_at 是首次点亮时间，
//! 分解只减 held 不灭灯（ever-lit 语义落在 0232 的列纪律上）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

#[get("/gacha/me")]
pub(super) async fn gacha_me(
    state: Db,
    req: HttpRequest,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let ticket: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT balance FROM gacha_ticket_balance \
         WHERE user_id = $1), 0)",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(0);
    let shard: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT balance FROM gacha_shard_balance \
         WHERE user_id = $1), 0)",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(0);
    let cards: Vec<(
        i64,
        String,
        String,
        String,
        i32,
        i32,
        i32,
        i32,
        i32,
        String,
    )> = sqlx::query_as(
        "SELECT c.id, c.key, c.name, c.rarity, uc.held, uc.lv::int, \
         c.lv_max::int, c.dupe_shards, c.synth_shards, \
         to_char(uc.lit_at, 'YYYY-MM-DD\"T\"HH24:MI:SS') \
         FROM gacha_user_cards uc JOIN gacha_cards c ON c.id = uc.card_id \
         WHERE uc.user_id = $1 ORDER BY c.sort, c.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let exchanged: Vec<i64> = sqlx::query_scalar(
        "SELECT card_id FROM gacha_exchanges WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let recent: Vec<(i32, String, Option<String>, i32, bool)> =
        sqlx::query_as(
            "SELECT seq, output_type, rarity, shards, was_pity \
             FROM gacha_draws WHERE user_id = $1 \
             ORDER BY id DESC LIMIT 30",
        )
        .bind(uid)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let lit_total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM gacha_cards WHERE enabled",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "ticketBalance": ticket,
        "shardBalance": shard,
        "cards": cards
            .iter()
            .map(|(id, key, name, rar, held, lv, lv_max, dupe, synth,
                  lit)| {
                serde_json::json!({
                    "id": id, "key": key, "name": name, "rarity": rar,
                    "held": held, "lv": lv, "lvMax": lv_max,
                    "dupeShards": dupe, "synthShards": synth,
                    "litAt": lit,
                    "exchanged": exchanged.contains(id),
                })
            })
            .collect::<Vec<_>>(),
        "litCount": cards.len(),
        "litTotal": lit_total,
        "recent": recent
            .iter()
            .map(|(seq, ty, rar, shards, wp)| {
                serde_json::json!({
                    "seq": seq, "type": ty, "r": rar,
                    "shards": shards, "wasPity": wp,
                })
            })
            .collect::<Vec<_>>(),
    })))
}

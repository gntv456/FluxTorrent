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
    // 等级门槛（0490）：与 POST /gacha/draw 同一把尺；公示面（rates/pool）
    // 保持匿名可读不拦——门槛锁的是玩，不是看
    crate::games_http::helpers::check_minclass(&state, "gacha", auth.class_id)
        .await?;
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
    // 碎片读数走共用尺子（P-361）：只在流水里有碎片的会员不该看到 0
    let shard = super::shards::shard_balance(&state.repo.db, uid).await?;
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
    // 抽卡记录页要「时间 / 保底 / 距硬保底」，此前只下发 seq 与档位，
    // 前端把 30 抽渲染成一条色带就没了下文。drawn_at 与 pity_at 本就在
    // gacha_draws 行上，随 me 一并下发，前端不再二次请求。
    #[allow(clippy::type_complexity)]
    let recent: Vec<(
        i32,
        String,
        Option<String>,
        i32,
        bool,
        String,
        i32,
        bool,
        i64,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT seq, output_type, rarity, shards, was_pity, \
             to_char(drawn_at AT TIME ZONE 'Asia/Shanghai', \
                 'YYYY-MM-DD\"T\"HH24:MI:SS'), \
             pity_at, is_new, banner_id, card_id \
             FROM gacha_draws WHERE user_id = $1 \
             ORDER BY id DESC LIMIT 30",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let lit_total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM gacha_cards WHERE enabled")
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
            .map(
                |(seq, ty, rar, shards, wp, at, pity, is_new, banner, card)| {
                    serde_json::json!({
                        "seq": seq, "type": ty, "r": rar,
                        "shards": shards, "wasPity": wp,
                        "drawnAt": at, "pityAt": pity,
                        "isNew": is_new, "bannerId": banner, "cardId": card,
                    })
                },
            )
            .collect::<Vec<_>>(),
    })))
}

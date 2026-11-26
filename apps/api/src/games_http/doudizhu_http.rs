//! 斗地主 HTTP 口（AI 房）：GET 局面 / POST 开局 / POST 动作。
//! 三端点都过等级门 + 每小时限次；开局扣底分、动作推进状态机，
//! 资金段全部在 `games::doudizhu_repo`（H 层禁 SQL 棘轮）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::doudizhu::DOU_DIZHU_RAKE;
use crate::games::doudizhu_repo::{
    apply_action_tx, load_active, load_last, start_game_tx, GameRow,
};
use crate::games::doudizhu_settle::Role;
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{check_bet, check_minclass, idem_key};
use super::pool::dberr;

/// 底分档位（固定三档；`games_max_bet` 在 check_bet 里另有一道总闸）。
pub(super) const STAKE_TIERS: [i64; 3] = [100, 500, 2000];

/// GameRow → 前端 JSON。手牌只发玩家自己的；AI 手牌只发张数。
pub(super) fn room_view(row: &GameRow) -> serde_json::Value {
    let g = &row.state;
    let landlord = g.landlord;
    let seats: Vec<serde_json::Value> = (0..3u8)
        .map(|s| {
            let is_me = s == 0;
            let role =
                landlord.map(|l| if l == s { "landlord" } else { "farmer" });
            serde_json::json!({
                "seat": s,
                "role": role,
                "count": g.hands[s as usize].len(),
                "cards": if is_me { g.hands[0].clone() } else { vec![] },
                // 癞子桌：只发玩家自己手的癞子编码，前端高亮标识。
                "wildcards": if is_me {
                    g.wildcards[0].clone()
                } else {
                    vec![]
                },
                "is_ai": s != 0,
            })
        })
        .collect();
    serde_json::json!({
        "id": row.id,
        "status": row.status,
        "phase": g.phase,
        "turn": g.turn,
        "my_turn": row.my_turn,
        "stake": row.stake,
        "multiplier": row.multiplier,
        "bomb_count": g.bombs,
        "bids": g.bids,
        "landlord": landlord,
        "kitty": if landlord.is_some() { g.kitty.clone() } else { vec![] },
        "last": g.last.as_ref().map(|c| serde_json::json!({
            "seat": g.last_seat, "kind": c.kind,
            "major": c.major.0, "cards": c.cards,
        })),
        "finished": g.finished,
        "result": row.result,
        "payout": row.payout,
        "net": row.net,
        "spring": row.spring,
        "reveal": g.reveal,
        "grabs": g.grabs,
        "discards": g.discards.clone(),
        "double_levels": g.double_levels,
        "grab_votes": g.grab_votes.clone(),
        "seats": seats,
        "events_tail": g.events.iter().rev().take(12).rev()
            .map(|e| serde_json::json!({
                "seat": e.seat, "action": e.action,
                "score": e.score, "cards": e.cards,
            }))
            .collect::<Vec<_>>(),
        // 结算回放用全量事件（行数有限：一局至多 ~60 手）
        "events": g.events.iter()
            .map(|e| serde_json::json!({
                "seat": e.seat, "action": e.action,
                "score": e.score, "cards": e.cards,
            }))
            .collect::<Vec<_>>(),
    })
}

/// 读设置键的斗地主节奏参数（缺省 20s/15s）。
async fn turn_seconds(state: &web::Data<std::sync::Arc<AppState>>) -> i64 {
    super::helpers::eco_i64(state, "doudizhu_turn_seconds", 20)
        .await
        .clamp(10, 60)
}

/// 挂机阈值（秒）：设置键 doudizhu_pvp_idle_minutes（分钟，0522）优先；
/// 未配置回落出牌时长 ×3（旧行为）。
async fn idle_seconds(state: &web::Data<std::sync::Arc<AppState>>) -> i64 {
    let mins = super::helpers::eco_i64(
        state,
        "doudizhu_pvp_idle_minutes",
        0, // 0 = 未配置 → 派生口径
    )
    .await;
    if mins > 0 {
        (mins * 60).clamp(60, 3600)
    } else {
        turn_seconds(state).await * 3
    }
}

#[get("/games/doudizhu/state")]
pub(super) async fn dz_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let active = load_active(&state.repo.db, auth.id).await?;
    let last = load_last(&state.repo.db, auth.id).await?;
    Ok(ok(serde_json::json!({
        "game": active.as_ref().map(room_view),
        "last": last.as_ref().map(room_view),
        "stake_tiers": STAKE_TIERS,
        "turn_seconds": turn_seconds(&state).await,
        "rake_permille": DOU_DIZHU_RAKE,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DzStartReq {
    stake: i64,
    /// 规则变体：classic（缺省）| wild（癞子）| nolook（不洗牌）。
    #[serde(default)]
    variant: String,
    /// 明牌：开局选择，倍数 ×2。
    #[serde(default)]
    reveal: bool,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/games/doudizhu/start")]
pub(super) async fn dz_start(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzStartReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    check_bet(&state, body.stake)
        .await
        .map_err(DomainError::Validation)?;
    if !STAKE_TIERS.contains(&body.stake) {
        return Err(DomainError::Validation(
            "底分档位仅支持 100/500/2000".into(),
        ));
    }
    // 已有活跃局不许再开
    if load_active(&state.repo.db, auth.id).await?.is_some() {
        return Err(DomainError::Validation(
            "已有进行中的对局，请先完成".into(),
        ));
    }
    let variant = match body.variant.as_str() {
        "" | "classic" => "classic",
        "wild" => "wild",
        "nolook" => "nolook",
        _ => {
            return Err(DomainError::Validation(
                "variant 仅支持 classic/wild/nolook".into(),
            ))
        }
    };
    let idem = idem_key("dz", auth.id, &body.idempotency_key);
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let row = start_game_tx(
        &mut tx,
        auth.id,
        body.stake,
        &idem,
        variant,
        body.reveal,
    )
    .await?;
    tx.commit().await.map_err(dberr)?;
    state.repo.audit(Some(auth.id), "game.doudizhu", None).await;
    Ok(ok(serde_json::json!({ "game": room_view(&row) })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DzActionReq {
    action: String, // "bid" | "play" | "hint"
    #[serde(default)]
    score: Option<i64>,
    #[serde(default)]
    cards: Vec<u8>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/games/doudizhu/action")]
pub(super) async fn dz_action(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let _ = idem_key("dz-act", auth.id, &body.idempotency_key);
    let Some(active) = load_active(&state.repo.db, auth.id).await? else {
        return Err(DomainError::Validation("没有进行中的对局".into()));
    };
    // 提示：只算不改状态（不动事务）。叫分阶段返回 hintBid，
    // 出牌阶段返回 hint —— 前端两条路径都消费（room-types.HintResp）。
    if body.action == "hint" {
        let g = &active.state;
        if g.phase == crate::games::doudizhu_game::Phase::Bidding {
            let bid = crate::games::doudizhu_game::GameState::suggested_bid(
                &g.hands[0],
            );
            return Ok(ok(serde_json::json!({ "hintBid": bid })));
        }
        let prev = match (&g.last, g.last_seat) {
            (Some(c), Some(s)) if s != 0 => Some(c),
            _ => None,
        };
        // 抢/弃/加倍阶段不出牌，不给出牌候选。
        if g.phase != crate::games::doudizhu_game::Phase::Playing {
            return Ok(ok(serde_json::json!({ "hint": null })));
        }
        let wilds = &g.wildcards[0];
        // 癞子座走枚举替身面的 wild_hint——模型/经典 ai_choose 都不懂癞子，
        // 会给压不过/被拒的牌。
        // 经典座优先 DouZero 模型建议（dz_choose_safe 内含状态机口径
        // 预校验 + 失败回落启发式）；模型缺失/编译开关关闭自动降级。
        let suggestion = if !wilds.is_empty() {
            crate::games::doudizhu_wild::wild_hint(&g.hands[0], wilds, prev)
        } else {
            crate::games::doudizhu_dz::dz_choose_safe(g, 0, false)
        };
        return Ok(ok(serde_json::json!({ "hint": suggestion })));
    }
    if !matches!(
        body.action.as_str(),
        "bid" | "grab" | "discard" | "double" | "play"
    ) {
        return Err(DomainError::Validation(
            "action 仅支持 bid/grab/discard/double/play/hint".into(),
        ));
    }
    // 动作必须轮到玩家（AI 段在 repo 内即时推进）
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let idle_secs = idle_seconds(&state).await;
    let row = apply_action_tx(
        &mut tx,
        auth.id,
        active.id,
        &body.action,
        body.score,
        &body.cards,
        idle_secs,
    )
    .await?;
    tx.commit().await.map_err(dberr)?;
    state.repo.audit(Some(auth.id), "game.doudizhu", None).await;
    Ok(ok(serde_json::json!({ "game": room_view(&row) })))
}

/// 观战视角脱敏：清空所有座位手牌/癞子，不标本人、不轮到行动；
/// 观战者只看桌面进度与各家剩牌张数，不泄露任何手牌。
pub(super) fn spectator_mask(v: &mut serde_json::Value) {
    v["my_turn"] = serde_json::json!(false);
    if let Some(seats) = v["seats"].as_array_mut() {
        for seat in seats.iter_mut() {
            seat["is_me"] = serde_json::json!(false);
            seat["cards"] = serde_json::json!(Vec::<i64>::new());
            seat["wildcards"] = serde_json::json!(Vec::<i64>::new());
        }
    }
}

/// 角色字符串（留作复盘口后续扩展用）。
#[allow(dead_code)]
fn role_str(r: Role) -> &'static str {
    match r {
        Role::Landlord => "landlord",
        Role::Farmer => "farmer",
    }
}

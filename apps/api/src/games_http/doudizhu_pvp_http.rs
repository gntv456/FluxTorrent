//! 斗地主 PvP 房间 HTTP 口：匹配进房 / 看房 / 退房 / 房主开局。
//! 房间只管撮合；牌局状态走每人一行镜像局（state.hands[0] 恒为本座
//! 手牌，前端零分支）。扣底分在开局事务里三人一起扣，缺一散桌。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::doudizhu_pvp_repo::{
    deal_room_tx, join_or_create_tx, leave_room_tx, set_ready_tx, touch_seen_db,
};
use crate::games::doudizhu_pvp_view::{list_waiting, load_room, my_room};
use crate::games::doudizhu_repo::load_active;
use crate::http::require_auth;
use crate::state::AppState;

use super::doudizhu_http::{room_view, STAKE_TIERS};
use super::helpers::{check_bet, check_minclass};
use super::pool::dberr;

/// 变体校验。
fn parse_variant(v: &str) -> Option<&'static str> {
    match v {
        "classic" => Some("classic"),
        "wild" => Some("wild"),
        "nolook" => Some("nolook"),
        _ => None,
    }
}

/// 房间 JSON（含座位用户名脱敏——只露座位与状态，不露他人 ID）。
fn room_view_json(
    room: &crate::games::doudizhu_pvp_view::RoomRow,
    my_seat: Option<i16>,
) -> serde_json::Value {
    serde_json::json!({
        "id": room.id,
        "variant": room.variant,
        "stake": room.stake,
        "status": room.status,
        "is_owner": false, // 由调用方按 uid 覆写
        "my_seat": my_seat,
        "seats": room.seats.iter().map(|(_id, seat, uid, ready, online)| {
            serde_json::json!({
                "seat": seat,
                "filled": true,
                "is_me": my_seat.is_some_and(|ms| ms == *seat),
                "ready": ready,
                "online": online,
                "user_id": if my_seat == Some(*seat) {
                    *uid
                } else {
                    0
                },
            })
        }).collect::<Vec<_>>(),
        "empty_seats": (0..3i16).filter(|s|
            !room.seats.iter().any(|(_, seat, _, _, _)| seat == s))
            .collect::<Vec<_>>(),
        "_owner_id": room.owner_id,
        "_seat_ids": room.seats.iter().map(|(id, ..)| id).collect::<Vec<_>>(),
    })
}

#[get("/games/doudizhu/pvp/rooms")]
pub(super) async fn dz_pvp_rooms(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<VariantQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let variant = parse_variant(q.variant.as_str()).unwrap_or("classic");
    let rooms = list_waiting(&state.repo.db, variant).await?;
    let mine = my_room(&state.repo.db, auth.id).await?;
    let my_seat = mine.as_ref().and_then(|r| {
        r.seats
            .iter()
            .find(|(_, _, uid, _, _)| *uid == auth.id)
            .map(|s| s.1)
    });
    Ok(ok(serde_json::json!({
        "rooms": rooms.iter().map(|r| serde_json::json!({
            "id": r.id,
            "variant": r.variant,
            "stake": r.stake,
            "players": r.seats.len(),
            "owner": r.owner_id == auth.id,
        })).collect::<Vec<_>>(),
        "mine": mine.as_ref().map(|r| {
            let mut v = room_view_json(r, my_seat);
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
        "stake_tiers": STAKE_TIERS,
    })))
}

#[derive(Deserialize, Default)]
pub(super) struct VariantQ {
    #[serde(default)]
    variant: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DzJoinReq {
    stake: i64,
    #[serde(default)]
    variant: String,
}

#[post("/games/doudizhu/pvp/join")]
pub(super) async fn dz_pvp_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzJoinReq>,
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
    let variant = parse_variant(&body.variant).ok_or_else(|| {
        DomainError::Validation("variant 仅支持 classic/wild/nolook".into())
    })?;
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let (rid, seat, joined) =
        join_or_create_tx(&mut tx, auth.id, body.stake, variant).await?;
    tx.commit().await.map_err(dberr)?;
    touch_seen_db(&state.repo.db, auth.id).await?;
    state
        .repo
        .audit(Some(auth.id), "game.doudizhu_pvp", None)
        .await;
    let room = load_room(&state.repo.db, rid).await?;
    Ok(ok(serde_json::json!({
        "room_id": rid,
        "seat": seat,
        "joined": joined,
        "room": room.as_ref().map(|r| {
            let mut v = room_view_json(r, Some(seat));
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
    })))
}

#[post("/games/doudizhu/pvp/leave")]
pub(super) async fn dz_pvp_leave(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let left = leave_room_tx(&mut tx, auth.id).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(ok(serde_json::json!({ "left_room": left })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DzReadyReq {
    ready: bool,
}

/// 准备/取消准备（waiting 房内）。满三人且全部准备好，房主即可开局。
#[post("/games/doudizhu/pvp/ready")]
pub(super) async fn dz_pvp_ready(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzReadyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    set_ready_tx(&mut tx, auth.id, body.ready).await?;
    tx.commit().await.map_err(dberr)?;
    let room = my_room(&state.repo.db, auth.id).await?;
    let my_seat = room.as_ref().and_then(|r| {
        r.seats
            .iter()
            .find(|(_, _, uid, _, _)| *uid == auth.id)
            .map(|s| s.1)
    });
    Ok(ok(serde_json::json!({
        "ready": body.ready,
        "room": room.as_ref().map(|r| {
            let mut v = room_view_json(r, my_seat);
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
    })))
}

/// 房主开局：满员即发牌建三行镜像局；此时各座的活跃局即可通过
/// 既有 `/games/doudizhu/state` 读到。
#[post("/games/doudizhu/pvp/deal")]
pub(super) async fn dz_pvp_deal(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let room = my_room(&state.repo.db, auth.id).await?;
    let Some(room) = room else {
        return Err(DomainError::Validation("不在任何房间".into()));
    };
    if room.owner_id != auth.id {
        return Err(DomainError::Validation("只有房主可以开局".into()));
    }
    if room.status != "waiting" {
        return Err(DomainError::Validation("房间已开局".into()));
    }
    if room.seats.len() != 3 {
        return Err(DomainError::Validation("三人才可开局".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    deal_room_tx(&mut tx, room.id).await?;
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "game.doudizhu_pvp", None)
        .await;
    let game = load_active(&state.repo.db, auth.id).await?;
    Ok(ok(serde_json::json!({
        "dealt": true,
        "game": game.as_ref().map(room_view),
    })))
}

#[get("/games/doudizhu/pvp/state")]
pub(super) async fn dz_pvp_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    touch_seen_db(&state.repo.db, auth.id).await?;
    let room = my_room(&state.repo.db, auth.id).await?;
    let my_seat = room.as_ref().and_then(|r| {
        r.seats
            .iter()
            .find(|(_, _, uid, _, _)| *uid == auth.id)
            .map(|s| s.1)
    });
    let game = load_active(&state.repo.db, auth.id).await?;
    Ok(ok(serde_json::json!({
        "room": room.as_ref().map(|r| {
            let mut v = room_view_json(r, my_seat);
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
        "game": game.as_ref().map(room_view),
    })))
}

//! 斗地主 PvP 房间 HTTP 口：匹配进房 / 看房 / 退房 / 房主开局。
//! 房间只管撮合；牌局状态走每人一行镜像局（state.hands[0] 恒为本座
//! 手牌，前端零分支）。扣底分在开局事务里三人一起扣，缺一散桌。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games::doudizhu_pvp_repo::{
    deal_room_tx, is_seat_db, join_or_create_tx, leave_room_tx,
    leave_spectate_db, list_messages_db, post_message_tx, set_ready_tx,
    spectate_room_db, spectate_tx, touch_seen_db, MSG_BODY_MAX,
};
use crate::games::doudizhu_pvp_view::{list_waiting, load_room, my_room};
use crate::games::doudizhu_repo::{load_active, load_room_game_db};
use crate::http::require_auth;
use crate::state::AppState;

use super::doudizhu_http::{room_view, spectator_mask, STAKE_TIERS};
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

/// 房间 JSON（含座位用户名脱敏——只露座位与状态，不露他人 ID；余额仅本人座可见）。
fn room_view_json(
    room: &crate::games::doudizhu_pvp_view::RoomRow,
    my_seat: Option<i16>,
) -> serde_json::Value {
    serde_json::json!({
        "id": room.id,
        "variant": room.variant,
        "stake": room.stake,
        "status": room.status,
        "visibility": room.visibility,
        "spectators": room.spectators,
        "is_owner": false, // 由调用方按 uid 覆写
        "my_seat": my_seat,
        "seats": room.seats.iter().map(
            |(seat, uid, ready, online, username, avatar_url, title, balance)| {
                let is_me = my_seat.is_some_and(|ms| ms == *seat);
                serde_json::json!({
                    "seat": seat,
                    "filled": true,
                    "is_me": is_me,
                    "ready": ready,
                    "online": online,
                    "nickname": username.clone().unwrap_or_default(),
                    "avatar_url": avatar_url.clone(),
                    "title": title.clone(),
                    // 余额只露本人座；他人座为 null，不亮筹码。
                    "balance": if is_me { *balance } else { None::<i64> },
                    "user_id": if is_me { *uid } else { 0 },
                })
            }
        ).collect::<Vec<_>>(),
        "empty_seats": (0..3i16).filter(|s|
            !room.seats.iter().any(|(seat, ..)| seat == s))
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
            .find(|(_, uid, _, _, _, _, _, _)| *uid == auth.id)
            .map(|s| s.0)
    });
    Ok(ok(serde_json::json!({
        "rooms": rooms.iter().map(|r| serde_json::json!({
            "id": r.id,
            "variant": r.variant,
            "stake": r.stake,
            "status": r.status,
            "players": r.seats.len(),
            "spectators": r.spectators,
            "visibility": r.visibility,
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
    /// 私密房：true = private（不进公开匹配列表）；本轮不启用加入口令。
    #[serde(default)]
    private: bool,
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
    let visibility = if body.private { "private" } else { "public" };
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let (rid, seat, joined) =
        join_or_create_tx(&mut tx, auth.id, body.stake, variant, visibility)
            .await?;
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
            .find(|(_, uid, _, _, _, _, _, _)| *uid == auth.id)
            .map(|s| s.0)
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
    // 座位玩家读本人活跃座行；若无本人局但在观战，则回显所观房间的权威
    // 镜像行（手牌按旁观脱敏，不泄露任何座位手牌）。
    let mut room = my_room(&state.repo.db, auth.id).await?;
    let mut my_seat: Option<i16> = room.as_ref().and_then(|r| {
        r.seats
            .iter()
            .find(|(_, uid, _, _, _, _, _, _)| *uid == auth.id)
            .map(|s| s.0)
    });
    let mut game = load_active(&state.repo.db, auth.id).await?;
    let mut spectating = false;
    if game.is_none() {
        if let Some(rid) = spectate_room_db(&state.repo.db, auth.id).await? {
            room = load_room(&state.repo.db, rid).await?;
            my_seat = None;
            game = load_room_game_db(&state.repo.db, rid).await?;
            spectating = game.is_some();
        }
    }
    let game_json = match game.as_ref() {
        Some(g) => {
            let mut v = room_view(g);
            if spectating {
                spectator_mask(&mut v);
                v["spectating"] = serde_json::json!(true);
            }
            v
        }
        None => serde_json::Value::Null,
    };
    Ok(ok(serde_json::json!({
        "spectating": spectating,
        "room": room.as_ref().map(|r| {
            let mut v = room_view_json(r, my_seat);
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
        "game": game_json,
    })))
}

/// 解析发言/观战所属房间：显式 room_id 优先，其次本人活跃座，再次观战房。
async fn resolve_room(
    db: &sqlx::PgPool,
    uid: i64,
    explicit: Option<i64>,
) -> DomainResult<Option<i64>> {
    if let Some(r) = explicit {
        return Ok(Some(r));
    }
    if let Some(r) = my_room(db, uid).await? {
        return Ok(Some(r.id));
    }
    if let Some(r) = spectate_room_db(db, uid).await? {
        return Ok(Some(r));
    }
    Ok(None)
}

/// 查用户状态：0 正常 / 1 禁言 / 2 封禁（封禁已在鉴权层拦截，这里只挡禁言发言）。
async fn user_status(db: &sqlx::PgPool, uid: i64) -> DomainResult<i16> {
    let (s,): (Option<i16>,) =
        sqlx::query_as("SELECT status FROM users WHERE id = $1")
            .bind(uid)
            .fetch_one(db)
            .await
            .map_err(dberr)?;
    Ok(s.unwrap_or(0))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DzRoomReq {
    #[serde(default)]
    room_id: Option<i64>,
}

/// 加入观战：非座位身份旁观当前房，不占座不扣底分。
#[post("/games/doudizhu/pvp/spectate")]
pub(super) async fn dz_spectate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzRoomReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let rid = resolve_room(&state.repo.db, auth.id, body.room_id)
        .await?
        .ok_or_else(|| DomainError::Validation("不在任何房间".into()))?;
    let room = load_room(&state.repo.db, rid)
        .await?
        .ok_or_else(|| DomainError::Validation("房间不存在".into()))?;
    if room.status != "waiting" && room.status != "playing" {
        return Err(DomainError::Validation("房间已结束".into()));
    }
    if is_seat_db(&state.repo.db, rid, auth.id).await? {
        return Err(DomainError::Validation("已是座位玩家，无需观战".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    spectate_tx(&mut tx, rid, auth.id).await?;
    tx.commit().await.map_err(dberr)?;
    let room = load_room(&state.repo.db, rid).await?;
    Ok(ok(serde_json::json!({
        "spectating": rid,
        "room": room.as_ref().map(|r| {
            let mut v = room_view_json(r, None);
            v["is_owner"] = serde_json::json!(r.owner_id == auth.id);
            v
        }),
    })))
}

/// 退出观战。
#[post("/games/doudizhu/pvp/spectate/leave")]
pub(super) async fn dz_spectate_leave(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    leave_spectate_db(&state.repo.db, auth.id).await?;
    Ok(ok(serde_json::json!({ "left": true })))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DzMsgQuery {
    #[serde(default)]
    room_id: Option<i64>,
}

/// 拉房间频道最近消息（时间升序，便于前端追加）。
#[get("/games/doudizhu/pvp/messages")]
pub(super) async fn dz_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<DzMsgQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let rid = resolve_room(&state.repo.db, auth.id, q.room_id)
        .await?
        .ok_or_else(|| DomainError::Validation("不在任何房间".into()))?;
    let msgs = list_messages_db(&state.repo.db, rid, 50).await?;
    Ok(ok(serde_json::json!({
        "room_id": rid,
        "messages": msgs
            .into_iter()
            .map(|(id, sid, nickname, body, at)| {
                serde_json::json!({
                    "id": id,
                    "sender_id": sid,
                    "nickname": nickname,
                    "body": body,
                    "created_at": at.to_rfc3339(),
                })
            })
            .collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DzMsgReq {
    #[serde(default)]
    room_id: Option<i64>,
    #[serde(default)]
    body: String,
}

/// 发房间频道消息：仅座位或当前观战者可发；禁言拦截；正文清洗截断。
#[post("/games/doudizhu/pvp/messages")]
pub(super) async fn dz_message_post(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DzMsgReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "doudizhu", auth.class_id).await?;
    let rid = resolve_room(&state.repo.db, auth.id, body.room_id)
        .await?
        .ok_or_else(|| DomainError::Validation("不在任何房间".into()))?;
    let is_seat = is_seat_db(&state.repo.db, rid, auth.id).await?;
    let is_spec = spectate_room_db(&state.repo.db, auth.id)
        .await?
        .is_some_and(|r| r == rid);
    if !is_seat && !is_spec {
        return Err(DomainError::Forbidden);
    }
    if user_status(&state.repo.db, auth.id).await? == 1 {
        return Err(DomainError::Validation("账号已被禁言".into()));
    }
    let clean = body
        .body
        .trim()
        .chars()
        .take(MSG_BODY_MAX)
        .collect::<String>();
    if clean.is_empty() {
        return Err(DomainError::Validation("消息不能为空".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    post_message_tx(&mut tx, rid, auth.id, &clean).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(ok(serde_json::json!({ "sent": true })))
}

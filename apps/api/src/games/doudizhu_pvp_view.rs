//! 斗地主 PvP 房间读侧（房间/座位视图，从 doudizhu_pvp_repo.rs 拆出：
//! 主文件贴 300 行门禁）。只读查询，不含事务写。

use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 房间视图行（HTTP 层转 JSON）。repo 写侧经本模块引用。
pub(crate) struct RoomRow {
    pub id: i64,
    pub variant: String,
    pub stake: i64,
    pub status: String,
    pub owner_id: i64,
    pub seats: Vec<(i64, i16, i64, bool)>, // (seat, seat_no, user_id, ready)
}

/// 读房间 + 座位。
/// 读房间 + 座位。
pub(crate) async fn load_room(
    db: &sqlx::PgPool,
    rid: i64,
) -> DomainResult<Option<RoomRow>> {
    let head: Option<(i64, String, i64, String, i64)> = sqlx::query_as(
        "SELECT id, variant, stake, status, owner_id \
           FROM arcade_doudizhu_rooms WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    let Some((id, variant, stake, status, owner_id)) = head else {
        return Ok(None);
    };
    let seats: Vec<(i16, i64, bool)> = sqlx::query_as(
        "SELECT seat, user_id, ready FROM arcade_doudizhu_room_seats \
          WHERE room_id = $1 ORDER BY seat",
    )
    .bind(rid)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    Ok(Some(RoomRow {
        id,
        variant,
        stake,
        status,
        owner_id,
        seats: seats
            .into_iter()
            .map(|(seat, user_id, ready)| {
                (i64::from(seat), seat, user_id, ready)
            })
            .collect(),
    }))
}
/// 玩家的活跃房间（含座位）。
/// 玩家的活跃房间（含座位）。
pub(crate) async fn my_room(
    db: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<Option<RoomRow>> {
    let Some((rid,)): Option<(i64,)> = sqlx::query_as(
        "SELECT s.room_id FROM arcade_doudizhu_room_seats s \
           JOIN arcade_doudizhu_rooms r ON r.id = s.room_id \
          WHERE s.user_id = $1 AND s.active \
            AND r.status IN ('waiting', 'playing') \
          ORDER BY s.room_id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(db)
    .await
    .map_err(dberr)?
    else {
        return Ok(None);
    };
    load_room(db, rid).await
}
/// 公开等待房列表（大厅匹配面板）。
/// 公开等待房列表（大厅匹配面板）。
pub(crate) async fn list_waiting(
    db: &sqlx::PgPool,
    variant: &str,
) -> DomainResult<Vec<RoomRow>> {
    let heads: Vec<(i64, String, i64, String, i64)> = sqlx::query_as(
        "SELECT id, variant, stake, status, owner_id \
           FROM arcade_doudizhu_rooms \
          WHERE status = 'waiting' AND variant = $1 \
          ORDER BY created_at LIMIT 20",
    )
    .bind(variant)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let mut out = Vec::new();
    for (id, variant, stake, status, owner_id) in heads {
        let seats: Vec<(i16, i64, bool)> = sqlx::query_as(
            "SELECT seat, user_id, ready FROM arcade_doudizhu_room_seats \
              WHERE room_id = $1 ORDER BY seat",
        )
        .bind(id)
        .fetch_all(db)
        .await
        .map_err(dberr)?;
        out.push(RoomRow {
            id,
            variant,
            stake,
            status,
            owner_id,
            seats: seats
                .into_iter()
                .map(|(seat, user_id, ready)| {
                    (i64::from(seat), seat, user_id, ready)
                })
                .collect(),
        });
    }
    Ok(out)
}

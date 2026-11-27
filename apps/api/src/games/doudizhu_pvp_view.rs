//! 斗地主 PvP 房间读侧（房间/座位视图，从 doudizhu_pvp_repo.rs 拆出：
//! 主文件贴 300 行门禁）。只读查询，不含事务写。

use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 座位视图行：(seat, user_id, ready, online, username, avatar_url, title, balance)。
/// balance 为座位主本人魔力余额；HTTP 序列化时仅对本人座露余额，他人座置 0。
pub(crate) type SeatRow = (
    i16,
    i64,
    bool,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

/// 房间视图行（HTTP 层转 JSON）。repo 写侧经本模块引用。
pub(crate) struct RoomRow {
    pub id: i64,
    pub variant: String,
    pub stake: i64,
    pub status: String,
    pub owner_id: i64,
    pub visibility: String,
    pub spectators: i64,
    pub seats: Vec<SeatRow>,
}

/// 座位在线判定窗口（秒）：last_seen 在该窗口内视为在线。
pub(crate) const ONLINE_WINDOW_SECS: i64 = 30;

/// 读房间座位（join users 取昵称/头像/称号/余额；左连接兜底空座/已注销）。
async fn seat_rows(db: &sqlx::PgPool, rid: i64) -> DomainResult<Vec<SeatRow>> {
    sqlx::query_as(
        "SELECT s.seat, s.user_id, s.ready, \
                (now() - s.last_seen < make_interval(secs => $2)) AS online, \
                COALESCE(u.username::text, ''), u.avatar_url, u.title, u.spark_balance \
           FROM arcade_doudizhu_room_seats s \
           LEFT JOIN users u ON u.id = s.user_id \
          WHERE s.room_id = $1 ORDER BY s.seat",
    )
    .bind(rid)
    .bind(ONLINE_WINDOW_SECS)
    .fetch_all(db)
    .await
    .map_err(dberr)
}

/// 观战人数（有效观战行数）。
async fn count_spectators(db: &sqlx::PgPool, rid: i64) -> DomainResult<i64> {
    let (n,): (Option<i64>,) = sqlx::query_as(
        "SELECT count(*)::bigint FROM arcade_doudizhu_spectators WHERE room_id = $1",
    )
    .bind(rid)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(n.unwrap_or(0))
}

/// 读房间 + 座位 + 观战数。
pub(crate) async fn load_room(
    db: &sqlx::PgPool,
    rid: i64,
) -> DomainResult<Option<RoomRow>> {
    let head: Option<(i64, String, i64, String, i64, String)> = sqlx::query_as(
        "SELECT id, variant, stake, status, owner_id, visibility \
           FROM arcade_doudizhu_rooms WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    let Some((id, variant, stake, status, owner_id, visibility)) = head else {
        return Ok(None);
    };
    let seats = seat_rows(db, rid).await?;
    let spectators = count_spectators(db, rid).await?;
    Ok(Some(RoomRow {
        id,
        variant,
        stake,
        status,
        owner_id,
        visibility,
        spectators,
        seats,
    }))
}

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

/// 公开房间列表（大厅匹配面板）：private 房不进公开列表；waiting 可入座，
/// playing 仅供旁观（列表带 status 供前端区分入口口径）。
pub(crate) async fn list_waiting(
    db: &sqlx::PgPool,
    variant: &str,
) -> DomainResult<Vec<RoomRow>> {
    let heads: Vec<(i64, String, i64, String, i64, String)> = sqlx::query_as(
        "SELECT id, variant, stake, status, owner_id, visibility \
           FROM arcade_doudizhu_rooms \
          WHERE status IN ('waiting', 'playing') AND variant = $1 AND visibility = 'public' \
          ORDER BY created_at LIMIT 20",
    )
    .bind(variant)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let mut out = Vec::new();
    for (id, variant, stake, status, owner_id, visibility) in heads {
        let seats = seat_rows(db, id).await?;
        let spectators = count_spectators(db, id).await?;
        out.push(RoomRow {
            id,
            variant,
            stake,
            status,
            owner_id,
            visibility,
            spectators,
            seats,
        });
    }
    Ok(out)
}

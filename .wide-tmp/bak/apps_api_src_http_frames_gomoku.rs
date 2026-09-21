//! 头像挂件 + 五子棋（插件区后半）。
//! 从 http.rs 按域拆出。

use crate::economy_http::spend_spark;
use actix_web::{get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;
use super::plugins::FrameRow;

#[get("/avatar-frames")]
pub async fn frame_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<FrameRow> = sqlx::query_as(
        "SELECT id, name, css, image_url, price FROM avatar_frames ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FrameSetBody {
    frame_id: Option<i32>,
}

/// 佩戴头像挂件（需已购买：简化口径 price=0 免费 / >0 扣魔力）
#[put("/me/avatar-frame")]
pub async fn frame_equip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FrameSetBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    match body.frame_id {
        Some(fid) => {
            let price: Option<i32> = sqlx::query_scalar(
                "SELECT price FROM avatar_frames WHERE id=$1",
            )
            .bind(fid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(price) = price else {
                return Err(DomainError::NotFound(fid as i64));
            };
            if price > 0 {
                let idem = format!("frame:{}:{}", auth.id, fid);
                // 幂等键确定性（frame:{uid}:{fid}）：换戴回已购框架 → Replayed（不重复
                // 扣款）后仍继续佩戴，这正是「重复佩戴不重复收费」的预期语义，显式丢弃。
                let outcome = spend_spark(
                    &state.repo.db,
                    auth.id,
                    price as i64,
                    "shop",
                    &idem,
                    "avatar_frame",
                    fid as i64,
                )
                .await?;
                let _ = outcome;
            }
            sqlx::query("UPDATE users SET avatar_frame_id=$2 WHERE id=$1")
                .bind(auth.id)
                .bind(fid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": fid })))
        }
        None => {
            sqlx::query("UPDATE users SET avatar_frame_id=NULL WHERE id=$1")
                .bind(auth.id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": null })))
        }
    }
}

// ---- 五子棋（wuziqi 口径：建房/加入/落子/棋盘）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct GomokuGame {
    id: i32,
    black_id: i64,
    white_id: Option<i64>,
    board: String,
    turn: String,
    winner_id: Option<i64>,
}

#[post("/gomoku/games")]
pub async fn gomoku_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gomoku_games (black_id, board) VALUES ($1, '') RETURNING id",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[post("/gomoku/games/{id}/join")]
pub async fn gomoku_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE gomoku_games SET white_id=$2, updated_at=now() WHERE id=$1 AND white_id IS NULL AND black_id <> $2",
    ).bind(*path).bind(auth.id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::Validation("对局不存在或已有对手".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(Deserialize)]
struct MoveBody {
    pos: i32,
}

#[post("/gomoku/games/{id}/move")]
pub async fn gomoku_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<MoveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(0..225).contains(&body.pos) {
        return Err(DomainError::Validation("落点越界".into()));
    }
    let g: Option<(i64, Option<i64>, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((black, white, board, turn, winner)) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if winner.is_some() {
        return Err(DomainError::Validation("对局已结束".into()));
    }
    let Some(white) = white else {
        return Err(DomainError::Validation("等待对手加入".into()));
    };
    let my_color = if auth.id == black {
        'b'
    } else if auth.id == white {
        'w'
    } else {
        return Err(DomainError::Forbidden);
    };
    if my_color.to_string() != turn {
        return Err(DomainError::Validation("还没轮到你".into()));
    }
    // 棋盘惰性填充
    let mut cells: Vec<char> = board.chars().collect();
    cells.resize(225, '.');
    if cells[body.pos as usize] != '.' {
        return Err(DomainError::Validation("该点已有棋子".into()));
    }
    cells[body.pos as usize] = my_color;
    let new_board: String = cells.iter().collect();
    let next_turn = if my_color == 'b' { "w" } else { "b" };
    // 胜负判定（四方向五连）
    let won = check_gomoku_win(&cells, body.pos as usize, my_color);
    let winner_id = if won { Some(auth.id) } else { None };
    sqlx::query(
        "UPDATE gomoku_games SET board=$2, turn=$3, winner_id=$4, updated_at=now() WHERE id=$1",
    )
    .bind(*path)
    .bind(&new_board)
    .bind(next_turn)
    .bind(winner_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "board": new_board, "turn": next_turn, "winner": winner_id, "you_won": won }),
    ))
}

#[get("/gomoku/games/{id}")]
pub async fn gomoku_get(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let g: Option<GomokuGame> = sqlx::query_as(
        "SELECT id, black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(g) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    Ok(ok(g))
}

pub fn check_gomoku_win(cells: &[char], pos: usize, color: char) -> bool {
    const SIZE: usize = 15;
    let (r, c) = (pos / SIZE, pos % SIZE);
    let dirs: [(isize, isize); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];
    for (dr, dc) in dirs {
        let mut count = 1;
        for sign in [1, -1] {
            let mut step = 1;
            loop {
                let rr = r as isize + dr * step * sign;
                let cc = c as isize + dc * step * sign;
                if rr < 0
                    || rr >= SIZE as isize
                    || cc < 0
                    || cc >= SIZE as isize
                {
                    break;
                }
                if cells[rr as usize * SIZE + cc as usize] != color {
                    break;
                }
                count += 1;
                step += 1;
            }
        }
        if count >= 5 {
            return true;
        }
    }
    false
}

//! 组队契约——我的队伍/中途退出/招募列表（从 social_http.rs 按域拆出）。
//!
//! 退出惩罚口径（见 `_doc/契约失败流转与信誉.md`）：只针对「逃跑」不针对
//! 「尽力」；信誉只影响准入资格，不剥夺已得收益。发起/加入见 team.rs。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::endangered::{setting_i64, social_disabled};

/// 我的队伍（含成员与各自在契约期内的做种增量）。
#[get("/social/team/mine")]
async fn team_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if social_disabled(&state.repo.db).await {
        return Ok(ok(json!({ "enabled": false, "list": [] })));
    }

    type Row = (
        i64,
        i16,
        i32,
        Option<i64>,
        Option<String>,
        Option<serde_json::Value>,
        bool,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT t.id, t.status, t.team_size_max, r.torrent_id, tor.name, \
                (SELECT json_agg(json_build_object( \
                    'uid', m2.uid, 'username', u2.username, 'is_leader', m2.is_leader, \
                    'join_status', m2.join_status, \
                    'delta_seconds', GREATEST(COALESCE(s2.seeded_seconds, 0) - m2.seed_seconds_begin, 0) \
                 ) ORDER BY m2.is_leader DESC, m2.joined_at) \
                 FROM social_team_member m2 \
                 JOIN users u2 ON u2.id = m2.uid \
                 LEFT JOIN snatches s2 ON s2.user_id = m2.uid AND s2.torrent_id = r.torrent_id \
                 WHERE m2.team_id = t.id) AS members, \
                me.is_leader \
         FROM social_team t \
         JOIN social_team_member me ON me.team_id = t.id AND me.uid = $1 \
         LEFT JOIN resurrections r ON r.team_id = t.id \
         LEFT JOIN torrents tor ON tor.id = r.torrent_id \
         ORDER BY t.created_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(
            |(id, status, size_max, tid, name, members, viewer_is_leader)| {
                json!({
                    "team_id": id,
                    "status": status,
                    "team_size_max": size_max,
                    "torrent_id": tid,
                    "torrent_name": name,
                    "members": members.unwrap_or_else(|| json!([])),
                    // 前端据此决定是否显示「退出」（队长不能直接退出，需先处理队伍）
                    "viewer_is_leader": viewer_is_leader,
                })
            },
        )
        .collect();

    Ok(ok(json!({ "enabled": true, "list": list })))
}

#[derive(Deserialize)]
struct LeaveTeamReq {
    team_id: i64,
}

/// 中途退出契约：**只扣信誉，不清零已产生的收益，也不连坐其他成员**。
///
/// 设计口径（见 `_doc/契约失败流转与信誉.md`）：
///   * 惩罚只针对「逃跑」，不针对「尽力」——未达标但坚持到期的成员不受此罚
///   * 信誉只影响准入资格（能否发起契约 / 接高级契约），不剥夺已得收益
#[post("/social/team/leave")]
async fn team_leave(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LeaveTeamReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if social_disabled(&state.repo.db).await {
        return Err(DomainError::Validation("社交玩法未开放".into()));
    }

    let row: Option<(i16, i64)> = sqlx::query_as(
        "SELECT t.status, t.leader_uid FROM social_team t \
         JOIN social_team_member m ON m.team_id = t.id AND m.uid = $2 \
         WHERE t.id = $1 AND m.join_status IN (0, 1)",
    )
    .bind(body.team_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (status, leader) =
        row.ok_or_else(|| DomainError::Validation("你不在该队伍中".into()))?;
    if status != 0 && status != 1 {
        return Err(DomainError::Validation("该队伍已结束".into()));
    }
    if leader == auth.id {
        return Err(DomainError::Validation("队长不能直接退出".into()));
    }

    let delta =
        setting_i64(&state.repo.db, "social_rep_on_withdrawn", -15).await;
    let rep_min = setting_i64(&state.repo.db, "social_rep_min", 0).await;
    let rep_max = setting_i64(&state.repo.db, "social_rep_max", 2000).await;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let upd = sqlx::query(
        "UPDATE social_team_member SET join_status = 3, left_at = now() \
         WHERE team_id = $1 AND uid = $2 AND join_status IN (0, 1)",
    )
    .bind(body.team_id)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();

    if upd > 0 {
        // 首次建档时基准分 1000（与 social_reputation.score 默认值一致）
        sqlx::query(
            "INSERT INTO social_reputation (uid, score, withdrawn_count, updated_at) \
             VALUES ($1, 1000 + $2, 1, now()) \
             ON CONFLICT (uid) DO UPDATE SET \
               score = LEAST($3, GREATEST($4, social_reputation.score + $2)), \
               withdrawn_count = social_reputation.withdrawn_count + 1, \
               updated_at = now()",
        )
        .bind(auth.id)
        .bind(delta)
        .bind(rep_max)
        .bind(rep_min)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    state
        .repo
        .audit(Some(auth.id), "social.team_leave", Some(body.team_id))
        .await;

    Ok(ok(
        json!({ "team_id": body.team_id, "reputation_delta": delta }),
    ))
}

#[derive(Serialize, sqlx::FromRow)]
struct TeamRow {
    team_id: i64,
    torrent_id: i64,
    torrent_name: String,
    leader_name: String,
    member_count: i64,
    team_size_max: i32,
    deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    seeders: i32,
    /// 当前用户是否已在该队伍中（前端据此把「加入」换成「已加入」）
    joined: bool,
}

/// 招募中的队伍列表 —— 组队的前提是玩家能看到有哪些队伍可加。
#[get("/social/team/list")]
async fn team_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if social_disabled(&state.repo.db).await {
        return Ok(ok(json!({ "enabled": false, "list": [] })));
    }

    let rows: Vec<TeamRow> = sqlx::query_as(
        "SELECT st.id AS team_id, r.torrent_id, tor.name AS torrent_name, \
                u.username AS leader_name, \
                (SELECT count(*) FROM social_team_member m \
                 WHERE m.team_id = st.id AND m.join_status IN (0, 1)) AS member_count, \
                st.team_size_max, st.deadline_at, COALESCE(tor.seeders, 0) AS seeders, \
                EXISTS(SELECT 1 FROM social_team_member m2 \
                       WHERE m2.team_id = st.id AND m2.uid = $1 AND m2.join_status IN (0, 1)) AS joined \
         FROM social_team st \
         JOIN resurrections r ON r.team_id = st.id AND r.status = 'open' \
         JOIN torrents tor ON tor.id = r.torrent_id \
         JOIN users u ON u.id = st.leader_uid \
         WHERE st.status IN (0, 1) AND st.settled_at IS NULL \
         ORDER BY st.created_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(json!({ "enabled": true, "list": rows })))
}

//! 组队契约——发起与加入（从 social_http.rs 按域拆出）。
//!
//! 复用 resurrections 作为任务本体，组队只是给它挂上 team_id：单人任务
//! team_id IS NULL，组队任务指向 social_team。贡献口径：不用 spark_ledger
//! 反查（seeding_reward 逐小时聚合无 ref），改为加入时快照 snatches 累计值
//! 作基线、结算算增量——与 seed_preserve 同款口径，精度到单资源。
//! 我的队伍/退出/列表见 team_members.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::endangered::social_disabled;

// ============ 组队契约（把单人复活任务扩展为多人协作）============
//
// 复用 resurrections 作为任务本体（torrent_id 唯一、不可领自己发布的种、worker 每小时验收），
// 组队只是给它挂上 team_id：单人任务 team_id IS NULL，组队任务指向 social_team。
//
// 贡献口径（关键）：**不用 spark_ledger 反查** —— seeding_reward 按用户逐小时聚合落库、
// 无 ref_type/ref_id，无法定位到单个种子。改为加入时快照 snatches 的累计值作基线、
// 结算时算增量，与 seed_preserve 的 seed_time_begin / uploaded_begin 同款口径，
// 精度到单资源，天然杜绝"挂一堆无关种子刷契约贡献"。

#[derive(Deserialize)]
struct CreateTeamReq {
    torrent_id: i64,
    team_size_max: Option<i32>,
}

/// 发起组队：建队 + 认领该资源的复活任务 + 队长入队（记录贡献基线）。
#[post("/social/team/create")]
async fn team_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CreateTeamReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if social_disabled(&state.repo.db).await {
        return Err(DomainError::Validation("社交玩法未开放".into()));
    }
    let tid = body.torrent_id;

    // 与 resurrections/claim 同口径：须过审，且不能拯救自己发布的种
    let owner: i64 = sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::NotFound(tid))?;
    if owner == auth.id {
        return Err(DomainError::Validation("不能拯救自己发布的种子".into()));
    }

    // 注意：snatches.seeded_seconds 是 INT（非 BIGINT），显式 ::bigint 以便统一按 i64 读
    let base: Option<(i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(seeded_seconds, 0)::bigint, COALESCE(uploaded, 0) \
         FROM snatches WHERE user_id = $1 AND torrent_id = $2",
    )
    .bind(auth.id)
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (seed0, up0) = base.ok_or_else(|| {
        DomainError::Validation("你需要先下载该资源，才能参与保种协作".into())
    })?;

    // 注意：site_settings.value 经 ::int 得到的是 INT4，必须读 i32（读 i64 会在解码期报
    // mismatched types 并被静默吞掉，导致配置永远不生效）
    let hours: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'resurrection_hours'), 240)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let size_max = body.team_size_max.unwrap_or(5).clamp(2, 10);
    // 契约期限：到期未达标由 worker 的 social_team_expire 判失败（见 0103 迁移）
    let days: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'social_team_default_days'), 14)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let team_id: i64 = sqlx::query_scalar(
        "INSERT INTO social_team (leader_uid, status, team_size_max, deadline_at) \
         VALUES ($1, 1, $2, now() + make_interval(days => $3)) RETURNING id",
    )
    .bind(auth.id)
    .bind(size_max)
    .bind(days)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // torrent_id 唯一约束保证一个资源同时只有一个任务在跑
    let ins = sqlx::query(
        "INSERT INTO resurrections (torrent_id, user_id, required_hours, reward_sparks, team_id) \
         VALUES ($1, $2, $3, 5000, $4) ON CONFLICT (torrent_id) DO NOTHING",
    )
    .bind(tid)
    .bind(auth.id)
    .bind(hours)
    .bind(team_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if ins == 0 {
        // tx 随作用域 drop，自动回滚
        return Err(DomainError::Validation(
            "该资源已有进行中的复活任务".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO social_team_member \
         (team_id, uid, is_leader, join_status, seed_seconds_begin, uploaded_begin) \
         VALUES ($1, $2, TRUE, 1, $3, $4)",
    )
    .bind(team_id)
    .bind(auth.id)
    .bind(seed0)
    .bind(up0)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    state
        .repo
        .audit(Some(auth.id), "social.team_create", Some(tid))
        .await;

    Ok(ok(json!({
        "team_id": team_id,
        "torrent_id": tid,
        "required_hours": hours,
        "team_size_max": size_max,
        "seed_seconds_begin": seed0,
    })))
}

#[derive(Deserialize)]
struct JoinTeamReq {
    team_id: i64,
}

/// 加入队伍（记录贡献基线）。自愿加入，无邀请制；中途退出只影响信誉，不清零已产生收益。
#[post("/social/team/join")]
async fn team_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JoinTeamReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if social_disabled(&state.repo.db).await {
        return Err(DomainError::Validation("社交玩法未开放".into()));
    }

    let row: Option<(i64, i16, i32, i64)> = sqlx::query_as(
        "SELECT r.torrent_id, t.status, t.team_size_max, t.leader_uid \
         FROM social_team t LEFT JOIN resurrections r ON r.team_id = t.id \
         WHERE t.id = $1",
    )
    .bind(body.team_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (torrent_id, status, size_max, leader) =
        row.ok_or(DomainError::NotFound(body.team_id))?;
    if status != 0 && status != 1 {
        return Err(DomainError::Validation("该队伍已结束".into()));
    }
    if leader == auth.id {
        return Err(DomainError::Validation("你已经是这支队伍的队长".into()));
    }

    let cnt: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social_team_member WHERE team_id = $1 AND join_status IN (0, 1)",
    )
    .bind(body.team_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if cnt >= size_max as i64 {
        return Err(DomainError::Validation("队伍已满".into()));
    }

    let base: Option<(i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(seeded_seconds, 0)::bigint, COALESCE(uploaded, 0) \
         FROM snatches WHERE user_id = $1 AND torrent_id = $2",
    )
    .bind(auth.id)
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (seed0, up0) = base.ok_or_else(|| {
        DomainError::Validation("你需要先下载该资源，才能参与保种协作".into())
    })?;

    let ins = sqlx::query(
        "INSERT INTO social_team_member \
         (team_id, uid, is_leader, join_status, seed_seconds_begin, uploaded_begin) \
         VALUES ($1, $2, FALSE, 1, $3, $4) ON CONFLICT (team_id, uid) DO NOTHING",
    )
    .bind(body.team_id)
    .bind(auth.id)
    .bind(seed0)
    .bind(up0)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if ins == 0 {
        return Err(DomainError::Validation("你已在这支队伍中".into()));
    }

    state
        .repo
        .audit(Some(auth.id), "social.team_join", Some(body.team_id))
        .await;

    Ok(ok(
        json!({ "team_id": body.team_id, "torrent_id": torrent_id }),
    ))
}

//! 社交层（0102）：濒危预警雷达 / 组队契约 / 赛季 / 信誉。
//!
//! ## 与现有复活体系的关系（重要）
//!
//! FluxTorrent 已有完整的「死种复活」闭环（0073，U3D Graveyard 口径）：
//!   `ops_http.rs`  GET /resurrections · POST /resurrections/claim · GET /resurrections/mine
//!   条件：seeders = 0 且 30 天无 snatches 活动；不可领自己的种；奖励 5000 火花 + 免费券
//!
//! 本模块**不重复实现它**，只补一个它没有的环节：
//!
//! | | 现有 resurrections | 本模块 endangered |
//! |---|---|---|
//! | 对象 | 已死的资源（seeders = 0） | **濒危但有救的资源（seeders <= 阈值）** |
//! | 定位 | 死种复活（后置抢救） | **预警雷达（前置保种）** |
//! | 动作 | 认领并做种 required_hours | 只读展示 + 引导去保种 |
//!
//! 前置保种的边际成本远低于后置复活——资源一旦死到 0 做种者，能否救活取决于还有没有人
//! 留着完整数据。所以「还没死的时候提醒大家」比「死了再救」有价值得多，且实现成本极低。
//!
//! ## 复用而非重建
//!   做种激励   worker::jobs::seeding_reward
//!   保种结算   worker::jobs::preserve_settle / preserve_exit
//!   账本       spark_ledger（分区表，幂等键应用层先查后插）
//!   反作弊     cheat_events + users.status
//!   复活流程   resurrections（本模块只读它的状态用于展示「已有人在救」）

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 模块开关：module_social = 'no' 时关闭。
/// 缺省（未配置）视为关闭 —— 通用程序不应默认暴露社区玩法。
async fn social_disabled(db: &sqlx::PgPool) -> bool {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'module_social'")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| v == "no")
        .unwrap_or(true)
}

/// 读取整数型站点设置（缺省取 dft）
async fn setting_i64(db: &sqlx::PgPool, name: &str, dft: i64) -> i64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(dft)
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<i64>,
    size: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct EndangeredItem {
    torrent_id: i64,
    info_hash: String,
    name: String,
    seeders: i32,
    leechers: i32,
    times_completed: i32,
    size: i64,
    category_id: i32,
    age_days: f64,
    /// 是否已有进行中的复活任务（resurrections.status = 'open'）
    rescue_open: bool,
}

#[derive(Serialize)]
struct ListResp {
    enabled: bool,
    /// 濒危阈值（做种数 <= 该值列入雷达）
    endangered_seeders: i64,
    /// 健康阈值（对齐保种区移出口径），用于前端展示「距健康还差几个做种者」
    health_seeders: i64,
    page: i64,
    size: i64,
    total: i64,
    list: Vec<EndangeredItem>,
}

/// 濒危预警雷达（只读）。
///
/// 不建清单表：濒危是动态状态（今天濒危、明天可能已被保种），落表反而要处理时效性，
/// 直接实时查 torrents 即可。排序「越濒危越靠前」：做种数升序 → 完成数降序 → 体积降序。
#[get("/social/endangered/list")]
async fn endangered_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;

    let endangered_seeders = setting_i64(&state.repo.db, "social_endangered_seeders", 1).await;
    let health_seeders = setting_i64(&state.repo.db, "social_health_seeders", 7).await;

    // 模块关闭：返回 enabled=false + 空列表（前端隐藏入口，不报错、不引导）
    if social_disabled(&state.repo.db).await {
        return Ok(ok(ListResp {
            enabled: false,
            endangered_seeders,
            health_seeders,
            page: 1,
            size: 20,
            total: 0,
            list: Vec::new(),
        }));
    }

    let page = q.page.unwrap_or(1).max(1);
    let size = q.size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * size;

    // count 与列表必须同套谓词，否则总数与实际行数不符
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents t \
         WHERE t.approval_status = 1 AND t.seeders <= $1 AND t.seeders > 0 AND t.times_completed >= 3",
    )
    .bind(endangered_seeders)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let list: Vec<EndangeredItem> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.info_hash, t.name, t.seeders, t.leechers, \
                t.times_completed, t.size, t.category_id, \
                (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::float8 AS age_days, \
                (r.id IS NOT NULL) AS rescue_open \
         FROM torrents t \
         LEFT JOIN resurrections r ON r.torrent_id = t.id AND r.status = 'open' \
         WHERE t.approval_status = 1 AND t.seeders <= $1 AND t.seeders > 0 AND t.times_completed >= 3 \
         ORDER BY t.seeders ASC, t.times_completed DESC, t.size DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(endangered_seeders)
    .bind(size)
    .bind(offset)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(ListResp {
        enabled: true,
        endangered_seeders,
        health_seeders,
        page,
        size,
        total,
        list,
    }))
}

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
    let owner: i64 =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1")
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
    let (seed0, up0) =
        base.ok_or_else(|| DomainError::Validation("你需要先下载该资源，才能参与保种协作".into()))?;

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
        return Err(DomainError::Validation("该资源已有进行中的复活任务".into()));
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
    let (torrent_id, status, size_max, leader) = row.ok_or(DomainError::NotFound(body.team_id))?;
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
    let (seed0, up0) =
        base.ok_or_else(|| DomainError::Validation("你需要先下载该资源，才能参与保种协作".into()))?;

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
    let (status, leader) = row.ok_or_else(|| DomainError::Validation("你不在该队伍中".into()))?;
    if status != 0 && status != 1 {
        return Err(DomainError::Validation("该队伍已结束".into()));
    }
    if leader == auth.id {
        return Err(DomainError::Validation("队长不能直接退出".into()));
    }

    let delta = setting_i64(&state.repo.db, "social_rep_on_withdrawn", -15).await;
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

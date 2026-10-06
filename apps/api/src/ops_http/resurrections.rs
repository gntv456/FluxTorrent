//! 死种复活（0073 Graveyard）：列表/认领/我的。
//! 从 ops_http/preserve.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/resurrections")]
pub(super) async fn resurrection_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?; // 登录可见（未过审不可见由 approval_status 保证）
                                                   // 审计修复（P2）：旧版 ORDER BY t.created_at ASC 恒取最老的 50 条——新死种永远
                                                   // 排不进列表（30 天活动排除口径下“最老”几乎全是无人问津的陈种）。改为按
                                                   // “最近一次有人活跃的时间”倒序：新鲜死种（刚断种、还有救的价值）优先露出。
                                                   // 0284 P1-5：契约前置可见——行内直接带 required_hours/reward_sparks，
                                                   // 认领前用户就能看到「要做多久、给多少」（此前只在 claim 响应里才揭晓）
    let rows: Vec<ResRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.size, \
                COALESCE((SELECT value::int FROM site_settings WHERE \
                 name = 'resurrection_hours'), 240) AS required_hours, \
                5000::bigint AS reward_sparks \
         FROM torrents t \
         WHERE t.approval_status = 1 AND t.seeders = 0 \
           AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.torrent_id = t.id \
                           AND s.last_seen_at > now() - interval '30 days') \
           AND NOT EXISTS (SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id AND r.status = 'open') \
         ORDER BY COALESCE((SELECT max(s.last_seen_at) FROM snatches s WHERE s.torrent_id = t.id), \
                           t.created_at) DESC, t.id DESC \
         LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 列表行（0284 P1-5 起为对象形态；旧元组 [id,name,size] 消费方为前端
/// resurrection-panel.tsx，同步升级）
#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct ResRow {
    torrent_id: i64,
    name: String,
    size: i64,
    required_hours: i32,
    reward_sparks: i64,
}

#[derive(Deserialize)]
struct ResClaimReq {
    torrent_id: i64,
}

/// 领取复活任务（一种一任务，CAS 防并发双领）；required_hours 领取时快照（可配）。
#[post("/resurrections/claim")]
pub(super) async fn resurrection_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 领取门槛：不能救自己的种（U3D 口径——自己的死种自己救没有增量价值）
    let owner: i64 = sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::NotFound(body.torrent_id))?;
    if owner == auth.id {
        return Err(DomainError::Validation("不能领取自己发布的种子".into()));
    }
    // value::int 得到的是 INT4，必须读 i32。原为 i64：解码期报 mismatched types，
    // 又被随后的 unwrap_or(240) 静默吞掉 —— 表现为 resurrection_hours 配置永远不生效、恒为 240。
    let hours: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'resurrection_hours'), 240)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(240);
    let inserted = sqlx::query(
        "INSERT INTO resurrections (torrent_id, user_id, required_hours, reward_sparks) \
         VALUES ($1, $2, $3, 5000) ON CONFLICT (torrent_id) DO NOTHING",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(hours)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if inserted == 0 {
        return Err(DomainError::Validation(
            "该种子已有进行中的复活任务".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "resurrection.claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id,
        "required_hours": hours,
        "reward": "5000 魔力 + 1 枚免费券 + 该种 7 天免费",
        "note": "worker 每小时自动验收：累计做种达标且当前仍在做种即发奖"
    })))
}

/// 我的复活任务
#[get("/resurrections/mine")]
pub(super) async fn resurrection_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, i64, i32, chrono::DateTime<chrono::Utc>, String, i32)> = sqlx::query_as(
        "SELECT r.torrent_id, r.reward_sparks, r.required_hours, r.claimed_at, r.status, \
                COALESCE(s.seeded_seconds, 0) \
         FROM resurrections r \
         LEFT JOIN snatches s ON s.user_id = r.user_id AND s.torrent_id = r.torrent_id \
         WHERE r.user_id = $1 ORDER BY r.claimed_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

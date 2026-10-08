use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/offers")]
pub(super) async fn offer_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OfferCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 仅已过审种子可提名候选（审计修复：旧版可对待审/被拒/软删种子发起，
    // promote 会直接置 approval_status=1 + official_tag 绕过审核流）
    let t_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND \
         approval_status = 1)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在或未过审".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO offers (user_id, torrent_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct OfferRow {
    id: i64,
    username: Option<String>,
    torrent_id: i64,
    torrent_name: Option<String>,
    votes: i32,
    promoted: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/offers")]
pub(super) async fn offer_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, OfferRow>(
        "SELECT o.id, u.username, o.torrent_id, t.name AS torrent_name, o.votes, o.promoted, o.created_at \
         FROM offers o LEFT JOIN users u ON u.id = o.user_id \
         LEFT JOIN torrents t ON t.id = o.torrent_id \
         WHERE NOT o.promoted ORDER BY o.votes DESC, o.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct OfferVoteReq {
    offer_id: i64,
}

#[post("/offers/vote")]
pub(super) async fn offer_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OfferVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 前置校验存在性与转正态（原实现对不存在 offer 先占位→扣款触发 FK 500；
    // 对已转正 offer 扣款→回滚→404，报错语义混乱且浪费一轮账务）
    let offer_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM offers WHERE id = $1 AND NOT promoted)",
    )
    .bind(body.offer_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !offer_ok {
        return Err(DomainError::NotFound(body.offer_id));
    }
    // 先占位投票记录（原子判重，防重放刷票）
    let voted = sqlx::query(
        "INSERT INTO offer_votes (offer_id, user_id, \
     cost) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING",
    )
    .bind(body.offer_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if voted.rows_affected() == 0 {
        return Err(DomainError::Validation("已投过票啦".into()));
    }
    // 候选投票 1 火花（旧站口径）；扣款失败回滚占位
    let idem = format!("offer-vote:{}:{}", auth.id, body.offer_id);
    if let Err(e) = spend_spark(
        &state.repo.db,
        auth.id,
        1,
        "vote",
        &idem,
        "offer",
        body.offer_id,
    )
    .await
    {
        let _ = sqlx::query(
            "DELETE FROM offer_votes WHERE offer_id = $1 AND user_id = $2",
        )
        .bind(body.offer_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
        return Err(e);
    }
    let updated = sqlx::query(
        "UPDATE offers SET votes = votes + 1 WHERE id = $1 AND NOT promoted",
    )
    .bind(body.offer_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(body.offer_id));
    }
    Ok(ok(serde_json::json!({ "voted": true })))
}

#[derive(Deserialize)]
struct PromoteReq {
    offer_id: i64,
}

#[post("/offers/promote")]
pub(super) async fn offer_promote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 候选转正为管理操作
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::OFFERS_PROMOTE,
    )
    .await?;
    let tid: Option<i64> = sqlx::query_scalar(
        "UPDATE offers SET promoted = true WHERE id = $1 AND NOT \
         promoted RETURNING torrent_id",
    )
    .bind(body.offer_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(tid) = tid else {
        return Err(DomainError::NotFound(body.offer_id));
    };
    // 审计 2026-10-08 P2-5：转正是第二条「过审」入口，旧版绕过审核台的
    // 全部口径（自审拦截/操作日志），这里是补齐而非收走——offers 候选
    // 本身已过社区投票，转正仍应保留。写 approved_at 是它一直做对的那个。
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    if owner_id == Some(auth.id) {
        // 自审回滚：offers 行还没被其他动作动过，直接还原
        let _ = sqlx::query("UPDATE offers SET promoted = false WHERE id = $1")
            .bind(body.offer_id)
            .execute(&state.repo.db)
            .await;
        return Err(DomainError::Validation(
            "不能把自己的候选转正（利益冲突）".into(),
        ));
    }
    sqlx::query(
        "UPDATE torrents SET approval_status = 1, \
     approved_at = now() WHERE id = $1",
    )
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 0159：官种口径收口——转正不再裸写 official_tag 列，改打 official-kind
    // 标签（当前字典=「官种」），列由 sync_official_tag 派生，两种筛选口径恒一致
    let official_tag_ids: Vec<i32> = sqlx::query_scalar(
        "SELECT id FROM tag_dict \
         WHERE kind = 'official' AND scope = 'torrent' \
         ORDER BY id LIMIT 1",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    crate::torrents::apply_torrent_tags(
        &state.repo.db,
        tid,
        &official_tag_ids,
        (auth.id, auth.class_id as i16),
    )
    .await?;
    // 与审核台 decide 同口径：操作时间线 + 过审副作用（自动促销/组订阅推送）
    let _ = sqlx::query(
        "INSERT INTO torrent_operation_logs (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, 'offer_promote', $3)",
    )
    .bind(tid)
    .bind(auth.id)
    .bind(serde_json::json!({ "offer_id": body.offer_id }).to_string())
    .execute(&state.repo.db)
    .await;
    crate::admin_http::review_side_effects::apply_approval_side_effects(
        &state.repo.db,
        tid,
        auth.id,
    )
    .await;
    crate::http::bump_guard_ver(&state).await;
    state
        .repo
        .audit(Some(auth.id), "offer_promote", Some(tid))
        .await;
    Ok(ok(
        serde_json::json!({ "torrent_id": tid, "official": true }),
    ))
}

// ============ M17 字幕 ============

#[derive(Deserialize)]
struct OfferCreateReq {
    torrent_id: i64,
}

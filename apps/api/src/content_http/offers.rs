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
    let t_exists =
        crate::repo::offers::torrent_approved(&state.repo, body.torrent_id)
            .await;
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在或未过审".into()));
    }
    let id = crate::repo::offers::create_offer(
        &state.repo,
        auth.id,
        body.torrent_id,
    )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[get("/offers")]
pub(super) async fn offer_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows = crate::repo::offers::list_offers(&state.repo)
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
    let offer_ok =
        crate::repo::offers::voteable(&state.repo, body.offer_id).await;
    if !offer_ok {
        return Err(DomainError::NotFound(body.offer_id));
    }
    // 先占位投票记录（原子判重，防重放刷票）
    let voted = crate::repo::offers::place_vote(
        &state.repo,
        body.offer_id,
        auth.id,
    )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !voted {
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
        crate::repo::offers::drop_vote(&state.repo, body.offer_id, auth.id)
            .await;
        return Err(e);
    }
    let counted = crate::repo::offers::count_vote(&state.repo, body.offer_id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !counted {
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
    let tid = crate::repo::offers::claim_promote(&state.repo, body.offer_id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(tid) = tid else {
        return Err(DomainError::NotFound(body.offer_id));
    };
    // 审计 2026-10-08 P2-5：转正是第二条「过审」入口，旧版绕过审核台的
    // 全部口径（自审拦截/操作日志），这里是补齐而非收走——offers 候选
    // 本身已过社区投票，转正仍应保留。写 approved_at 是它一直做对的那个。
    let owner_id = crate::repo::offers::torrent_owner(&state.repo, tid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if owner_id == Some(auth.id) {
        // 自审回滚：offers 行还没被其他动作动过，直接还原
        crate::repo::offers::unpromote(&state.repo, body.offer_id).await;
        return Err(DomainError::Validation(
            "不能把自己的候选转正（利益冲突）".into(),
        ));
    }
    // P-317：转正没有权力把种「放回架上」。建候选时校验的是当时的
    // approval_status，过审之后还有六条路径会把种置成下架/驳回/待复审
    // （interact.rs:177/197、manage.rs:331、trump_admin.rs:103、
    // v4_http.rs:116、torrent_batch.rs:320）。旧写法无条件写回 1，
    // 等于用一条过期候选撤销审核台的裁决，还顺手升成官种。
    // 守卫写进 UPDATE 本身：查询与写入之间被下架也认（抢不到即拒）。
    let stamped = crate::repo::offers::stamp_approved(&state.repo, tid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !stamped {
        crate::repo::offers::unpromote(&state.repo, body.offer_id).await;
        return Err(DomainError::Validation(
            "该候选对应的种子当前不在架（已下架或未过审），\
             请先在审核台重新过审后再转正"
                .into(),
        ));
    }
    // 0159：官种口径收口——转正不再裸写 official_tag 列，改打 official-kind
    // 标签（当前字典=「官种」），列由 sync_official_tag 派生，两种筛选口径恒一致
    let official_tag_ids = crate::repo::offers::official_tag_ids(&state.repo)
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
    crate::repo::offers::log_promote(&state.repo, tid, auth.id, body.offer_id)
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

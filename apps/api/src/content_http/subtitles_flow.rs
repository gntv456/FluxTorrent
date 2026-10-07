//! 求字幕工作流（0148 C2/C3/C4）：认领/弃单/交稿/验收 + crew 分账 + free 联动。
//! 发起/凑赏/列表在 subtitles_requests.rs；结算核心（含分账）在
//! subtitles_close.rs（300 行门禁按域拆分）。

use actix_web::{post, web, HttpRequest, HttpResponse};

use super::subtitles_requests::SubReqClaimBody;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 认领（subtitle_workflow=1 时可用）：
/// CAS 0→3 + deadline 落库；可带 crew（C3，份额合计 ≤100、人数 ≤ subtitle_crew_max）。
/// crew 存储形态：[{user_id, share, role, accepted}]，accepted 由被邀人逐个确认。
#[post("/subtitles/requests/{id}/claim")]
pub(super) async fn subtitle_request_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<SubReqClaimBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let wf_on = super::subtitles_util::subtitle_setting(
        &state.repo.db,
        "subtitle_workflow",
        "0",
    )
    .await?
        == "1";
    if !wf_on {
        return Err(DomainError::Validation(
            "本站未启用字幕认领流程（直接上传交付即可）".into(),
        ));
    }
    // F3：弃单禁令拦截——audit_log 数 30 天内 subreq.abandon ≥ 3 次即禁
    // （abandon 端点只发通知不拦，禁令在此处真正生效；staff 不受限）。
    if auth.class_id < 90 {
        let recent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND action = \
             'subreq.abandon' AND created_at > now() - interval '30 days'",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if recent >= 3 {
            return Err(DomainError::Validation(
                "你近期多次弃单，30 天内暂停认领求字幕".into(),
            ));
        }
        // B1：并发在身单数上限——防霸住一串单坐等行情；staff 不受限
        let open: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM subtitle_requests \"
             WHERE claimed_by = $1 AND status IN (3, 4)",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if open >= 3 {
            return Err(DomainError::Validation(
                "你在身的求字幕已达 3 单，请先交稿或弃单后再认领".into(),
            ));
        }
    }
    // crew 校验（C3）
    let crew_max: i32 = super::subtitles_util::subtitle_setting(
        &state.repo.db,
        "subtitle_crew_max",
        "4",
    )
    .await?
    .parse()
    .unwrap_or(4);
    let crew = body.crew.as_deref().unwrap_or(&[]);
    if crew.len() as i32 > crew_max {
        return Err(DomainError::Validation(format!(
            "协作队伍最多 {crew_max} 人（不含主认领人）"
        )));
    }
    let share_sum: i64 = crew.iter().map(|c| c.share).sum();
    if share_sum > 100 {
        return Err(DomainError::Validation(
            "协作份额合计不能超过 100%".into(),
        ));
    }
    for c in crew {
        if !(1..=100).contains(&c.share) {
            return Err(DomainError::Validation(
                "协作份额需在 1-100 之间".into(),
            ));
        }
        if c.user_id == auth.id {
            return Err(DomainError::Validation(
                "不能把自己写进协作队伍".into(),
            ));
        }
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
        )
        .bind(c.user_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !exists {
            return Err(DomainError::Validation(format!(
                "用户 #{} 不存在",
                c.user_id
            )));
        }
    }
    let timeout_days: i32 = super::subtitles_util::subtitle_setting(
        &state.repo.db,
        "subtitle_claim_timeout_days",
        "7",
    )
    .await?
    .parse()
    .unwrap_or(7);
    let timeout = (timeout_days.max(1) as f64) as i32;
    // 认领主体：无 crew → 直接生效（accepted 全 true）；
    // 有 crew → 全员 pending，被邀人逐个 accept 后才进入翻译期（此简化版：
    // 认领即生效、crew 确认只影响结算分配，未确认者结算时份额归主认领人）
    let crew_json: serde_json::Value = if crew.is_empty() {
        serde_json::json!([])
    } else {
        serde_json::json!(crew
            .iter()
            .map(|c| serde_json::json!({
                "user_id": c.user_id, "share": c.share,
                "role": c.role, "accepted": false,
            }))
            .collect::<Vec<_>>())
    };
    let row: Option<(i64, i64)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 3, claimed_by = $2, \
         claimed_at = now(), deadline_at = now() + ($4 || ' days')::interval, \
         crew = $3 WHERE id = $1 AND status = 0 RETURNING user_id, bounty",
    )
    .bind(rid)
    .bind(auth.id)
    .bind(&crew_json)
    .bind(timeout.to_string())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, bounty)) = row else {
        return Err(DomainError::Validation("该求字幕不存在或已被认领".into()));
    };
    if owner_id == auth.id {
        // 回滚：发起人不能认领自己的单（防自薅悬赏入自己账）
        let _ = sqlx::query(
            "UPDATE subtitle_requests SET status = 0, claimed_by = NULL, \
             claimed_at = NULL, deadline_at = NULL, crew = '[]' WHERE id = $1",
        )
        .bind(rid)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::Validation("不能认领自己发起的求字幕".into()));
    }
    // 通知：发起人 + 被 crew 邀请者
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES (NULL, $1, '字幕翻译已被认领', $2)",
    )
    .bind(owner_id)
    .bind(format!(
        "你发起的求字幕 #{rid} 已被认领（赏金 {bounty}），认领人须在 {timeout} 天内交稿。"
    ))
    .execute(&state.repo.db)
    .await;
    for c in crew {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             VALUES (NULL, $1, '字幕翻译协作邀请', $2)",
        )
        .bind(c.user_id)
        .bind(format!(
            "用户 #{} 邀请你协作求字幕 #{rid}（角色 {}，份额 {}%）。结算按此份额分账。§§URL§§/subtitles",
            auth.id, c.role, c.share
        ))
        .execute(&state.repo.db)
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "subreq.claim", Some(rid))
        .await;
    Ok(ok(serde_json::json!({
        "id": rid, "deadline_days": timeout, "crew": crew_json,
    })))
}

/// 弃单（认领人本人）：3→0 回池；crew 清空；连续 3 次弃单禁认领 30 天。
#[post("/subtitles/requests/{id}/abandon")]
pub(super) async fn subtitle_request_abandon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let staff = auth.class_id >= 90;
    let n = sqlx::query(
        "UPDATE subtitle_requests SET status = 0, claimed_by = NULL, \
         claimed_at = NULL, deadline_at = NULL, crew = '[]' \
         WHERE id = $1 AND status = 3 AND ($2 OR claimed_by = $3)",
    )
    .bind(rid)
    .bind(staff)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "该求字幕不在认领状态或不在你名下".into(),
        ));
    }
    // 弃单计数：audit_log 里数本条 recent 弃单（粗粒度防霸单，不建新表）
    state
        .repo
        .audit(Some(auth.id), "subreq.abandon", Some(rid))
        .await;
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND action = \
         'subreq.abandon' AND created_at > now() - interval '30 days'",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let banned = recent >= 3;
    if banned {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             VALUES (NULL, $1, '字幕认领限制', \
             '你近期多次弃单，30 天内暂停认领求字幕。')",
        )
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
    }
    Ok(ok(serde_json::json!({
        "id": rid, "released": true,
        "abandon_count_30d": recent, "claim_banned": banned,
    })))
}

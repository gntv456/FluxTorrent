//! M15 论坛悬赏（采纳发放）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct BountyAwardReq {
    topic_id: i64,
    post_id: i64,
}

/// 楼主采纳回复：悬赏发放给答主。约束：
/// · 仅 topic_type=bounty 且 bounty_status='open'（CAS 防并发双采）；
/// · 仅楼主本人（版主代采会引发「谁的钱谁做主」纠纷，不做）；
/// · 不能采楼主首帖（自己给自己发钱）；
/// · 发放幂等键锚定 topic：`forum-bounty-pay:{topic_id}`（一个悬赏只发一次）。
#[post("/forums/bounty/award")]
async fn bounty_award(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BountyAwardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let t: Option<(i64, i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, user_id, bounty_spark, bounty_status, topic_type FROM topics WHERE id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((tid, op, spark, bstatus, ttype)) = t else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    if ttype != "bounty" {
        return Err(DomainError::Validation("该主题不是悬赏帖".into()));
    }
    if auth.id != op {
        return Err(DomainError::Forbidden);
    }
    if bstatus != "open" {
        return Err(DomainError::Validation("悬赏已处理".into()));
    }
    // 目标楼必须属于本主题、非楼主首帖
    let p: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM posts WHERE id = $1 AND topic_id = $2",
    )
    .bind(body.post_id)
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(answerer) = p else {
        return Err(DomainError::Validation("目标回复不存在".into()));
    };
    if answerer == op {
        return Err(DomainError::Validation("不能采纳自己的首帖".into()));
    }
    // CAS：open → awarded（并发双采只成功一个）
    let n = sqlx::query(
        "UPDATE topics SET bounty_status = 'awarded', bounty_post_id = $2 \
         WHERE id = $1 AND bounty_status = 'open'",
    )
    .bind(tid)
    .bind(body.post_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 审计流水（钱以 spark_ledger 为准，此表只留「谁采了谁」）
    let _ = sqlx::query(
        "INSERT INTO topic_bounty_awards (topic_id, post_id, answerer_id, awarded_by, spark) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tid)
    .bind(body.post_id)
    .bind(answerer)
    .bind(auth.id)
    .bind(spark)
    .execute(&state.repo.db)
    .await;
    if spark > 0 {
        let idem = format!("forum-bounty-pay:{}", tid);
        earn_spark(&state.repo.db, answerer, spark, "forum_bounty", &idem)
            .await?;
    }
    // 双向通知：答主拿钱（后端拿不到 site_settings，货币名落默认口径「魔力」，与 economy_http 同约定）
    notify_user(
        &state.repo.db,
        answerer,
        "悬赏已发放",
        &format!(
            "您的回复被采纳，获得 {} 魔力悬赏：[/forums/topic/{}]",
            spark, tid
        ),
    )
    .await;
    state
        .repo
        .audit(Some(auth.id), "forum.bounty_award", Some(tid))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": tid, "post_id": body.post_id, "spark": spark }),
    ))
}

// ---- 论坛投票（0125）：范式照 fun_polls（0016）——选项 JSONB、一人一票 UNIQUE、ON CONFLICT 幂等 ----

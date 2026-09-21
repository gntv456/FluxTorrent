//! M15 论坛打赏（楼层打赏 post_tip）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct PostTipReq {
    post_id: i64,
    spark: i64,
    #[serde(default)]
    note: Option<String>,
}

/// 打赏某楼：任何登录用户可打赏任何非匿名楼的作者。约束：
/// · 金额 1~100,000 钳位外拒绝；
/// · 不能打赏自己的楼（自己转自己只是流水噪音）；
/// · 打赏人余额不足自然被 spend_spark 拒（409）；
/// · 幂等键 `forum-tip:{from}:{post}:{uuid}`——打赏是主动行为可重复（同一个人可以多次打赏同一楼），
///   幂等只防网络重试，不防故意多次。
#[post("/forums/tip")]
async fn post_tip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostTipReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.spark <= 0 || body.spark > 100_000 {
        return Err(DomainError::Validation(
            "打赏金额需在 1~100000 之间".into(),
        ));
    }
    let note: String = body
        .note
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .chars()
        .take(50)
        .collect();
    // 目标楼必须存在且属于某主题（匿名楼的 user_id 为 NULL 不可打赏）
    let p: Option<(i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT p.topic_id, p.user_id, t.forum_id FROM posts p JOIN topics t ON t.id = p.topic_id \
         WHERE p.id = $1",
    )
    .bind(body.post_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((topic_id, author, fid)) = p else {
        return Err(DomainError::NotFound(body.post_id));
    };
    let Some(author_id) = author else {
        return Err(DomainError::Validation("匿名帖不可打赏".into()));
    };
    if author_id == auth.id {
        return Err(DomainError::Validation("不能打赏自己".into()));
    }
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let idem =
        format!("forum-tip:{}:{}:{}", auth.id, body.post_id, Uuid::new_v4());
    // spend(打赏人) → earn(作者) 对冲；三写（spend/earn/落账）单事务——旧版 earn 失败
    // 时打赏已扣、作者未入账且 post_tips 无痕。earn 幂等键锚定 spend 的 uuid。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放闸门（#[must_use] 连审）：重放时 spend 不再扣款而 earn 照发 = 凭空入账
    if !matches!(
        crate::economy_http::spend_spark_tx(
            &mut tx,
            auth.id,
            body.spark,
            "forum_tip",
            &idem,
            "forum_tip",
            body.post_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔打赏已受理，请勿重复提交".into(),
        ));
    }
    // spend 已闸 Spent，earn 幂等键锚定本次新 spend id，必为 Spent；
    // 显式丢弃以满足 must_use 契约
    let earn_outcome = crate::economy_http::earn_spark_tx(
        &mut tx,
        author_id,
        body.spark,
        "forum_tip",
        &format!("{idem}:to"),
    )
    .await?;
    let _ = earn_outcome;
    sqlx::query(
        "INSERT INTO post_tips (post_id, topic_id, from_user, to_user, spark, note) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(body.post_id)
    .bind(topic_id)
    .bind(auth.id)
    .bind(author_id)
    .bind(body.spark)
    .bind(&note)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    notify_user(
        &state.repo.db,
        author_id,
        "收到打赏",
        &format!(
            "用户 #{} 打赏了您在 [/forums/topic/{}] 的回复：{} 魔力{}",
            auth.id,
            topic_id,
            body.spark,
            if note.is_empty() {
                String::new()
            } else {
                format!("（{note}）")
            }
        ),
    )
    .await;
    state
        .repo
        .audit(Some(auth.id), "forum.post_tip", Some(body.post_id))
        .await;
    Ok(ok(
        serde_json::json!({ "post_id": body.post_id, "spark": body.spark, "to": author_id }),
    ))
}

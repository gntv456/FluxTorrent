//! 趣味投票（M24 fun 玩法）。
//! 从 games_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/fun/polls")]
pub(super) async fn fun_polls(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<FunPollRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.options, p.closed,             (SELECT v.option_index FROM fun_votes v WHERE v.poll_id = p.id AND v.user_id = $1) AS my_vote,             (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS total          FROM fun_polls p WHERE NOT p.closed ORDER BY p.id LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 附每项计数
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let counts: Vec<(i32, i64)> = sqlx::query_as(
            "SELECT option_index, count(*) FROM fun_votes WHERE poll_id = $1 GROUP BY option_index",
        )
        .bind(r.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        out.push(serde_json::json!({
            "id": r.id, "question": r.question, "options": r.options,
            "closed": r.closed, "my_vote": r.my_vote, "total_votes": r.total,
            "counts": counts.into_iter().map(|(i, c)| serde_json::json!({"index": i, "votes": c})).collect::<Vec<_>>(),
        }));
    }
    Ok(ok(out))
}

#[derive(Deserialize)]
struct FunVoteReq {
    poll_id: i64,
    option_index: i32,
}

#[post("/fun/vote")]
pub(super) async fn fun_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 审计修复（P2 竞态）：旧版先 INSERT 占位、校验失败再 DELETE 回滚——并发下一人的
    // 合法占位可能被另一人的非法回滚误删。校验全部前置，通过后才落占位。
    let valid: Option<(serde_json::Value, bool)> =
        sqlx::query_as("SELECT options, closed FROM fun_polls WHERE id = $1")
            .bind(body.poll_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((options, closed)) = valid else {
        return Err(DomainError::Validation("投票不存在".into()));
    };
    let n = options.as_array().map(|a| a.len()).unwrap_or(0);
    if closed {
        return Err(DomainError::Validation("投票已结束".into()));
    }
    if body.option_index < 0 || body.option_index as usize >= n {
        return Err(DomainError::Validation("选项无效".into()));
    }
    let voted = sqlx::query(
        "INSERT INTO fun_votes (poll_id, user_id, option_index) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(body.poll_id)
    .bind(auth.id)
    .bind(body.option_index)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if voted.rows_affected() == 0 {
        return Err(DomainError::Validation("已经投过啦，一人一票".into()));
    }
    // 投票 +1 火花（旧站口径），扣款失败回滚占位
    let idem = format!("fun-vote:{}:{}", auth.id, body.poll_id);
    if let Err(e) = spend_spark(
        &state.repo.db,
        auth.id,
        1,
        "vote",
        &idem,
        "fun_poll",
        body.poll_id,
    )
    .await
    {
        let _ = sqlx::query(
            "DELETE FROM fun_votes WHERE poll_id = $1 AND user_id = $2",
        )
        .bind(body.poll_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
        return Err(e);
    }
    Ok(ok(
        serde_json::json!({ "voted": body.option_index, "cost": 1 }),
    ))
}

#[derive(sqlx::FromRow)]
struct FunPollRow {
    id: i64,
    question: String,
    options: serde_json::Value,
    closed: bool,
    my_vote: Option<i32>,
    total: i64,
}

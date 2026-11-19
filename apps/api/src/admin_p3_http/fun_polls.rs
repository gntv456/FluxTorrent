//! 官方投票建口（0503 运营台账 #6）：fun_polls 有 funboxvote 投票动作、
//! 有 admin 只读总览（POLLS_MANAGE），但**无建票/关票写口**——官方
//! 无法在后台发起投票，只能 SQL 打库。本批补两写口（POLLS_MANAGE），
//! 同时接线 funboxvote 死奖励卡：投票者按 site_settings 发火花
//! （每票一行幂等键，重放不发）。

use actix_web::{post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct FunPollReq {
    pub question: String,
    /// ["选项A", "选项B", ...]
    pub options: Vec<String>,
}

/// POST /admin/fun-polls —— 官方建票（POLLS_MANAGE）。
#[post("/admin/fun-polls")]
pub async fn fun_poll_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunPollReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::POLLS_MANAGE)
        .await?;
    let q = body.question.trim();
    let opts: Vec<&str> = body
        .options
        .iter()
        .map(|o| o.trim())
        .filter(|o| !o.is_empty())
        .collect();
    if q.is_empty() || opts.len() < 2 {
        return Err(DomainError::Validation(
            "问题与至少两个选项不能为空".into(),
        ));
    }
    let opts_json = serde_json::to_value(
        opts.iter().map(|o| o.to_string()).collect::<Vec<_>>(),
    )
    .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO fun_polls (question, options, created_by) \\
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(q)
    .bind(&opts_json)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "fun_poll.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// PUT /admin/fun-polls/{id}/close —— 关票（停止投票）。
#[put("/admin/fun-polls/{id}/close")]
pub async fn fun_poll_close(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::POLLS_MANAGE)
        .await?;
    let id = path.into_inner();
    sqlx::query("UPDATE fun_polls SET closed = TRUE WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "fun_poll.close", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

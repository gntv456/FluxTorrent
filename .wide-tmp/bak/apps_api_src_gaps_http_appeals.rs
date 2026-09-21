//! 申诉（0020）：提交/我的/队列/处理。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use redis::AsyncCommands;
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::captcha::captcha_verify;

// ============ 申诉 ============

#[derive(Deserialize)]
struct AppealReq {
    kind: String,
    #[serde(default)]
    ref_id: Option<i64>,
    body: String,
    /// 未登录封禁申诉（P0 修复）专用字段：被申诉的用户名 + 验证码
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    captcha_id: Option<String>,
    #[serde(default)]
    captcha_answer: Option<i32>,
}

/// 封禁申诉的验证码校验（复用注册同款图形验证码，一次性消费）
pub async fn captcha_check(
    state: &AppState,
    id: &Option<String>,
    answer: &Option<i32>,
) -> DomainResult<()> {
    let (Some(id), Some(answer)) = (id.as_deref(), *answer) else {
        return Err(DomainError::Validation("请填写图形验证码".into()));
    };
    if !captcha_verify(state, id, answer).await {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    Ok(())
}

#[post("/appeals")]
pub async fn appeal_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealReq>,
) -> DomainResult<HttpResponse> {
    // 已登录用户：常规提交路径。
    // 审计修复（P0）：封禁申诉的目标人群（status>=2）登录被拒、require_auth 返回 403，
    // 旧版对该人群完全不可用。现在未携带有效凭证时按「被封申诉」专用分支处理：
    // 凭用户名 + 预验证码 + 防刷限流即可提交，不签发任何会话（见下方 guest 分支）。
    let auth = require_auth(&req, &state).await;
    let user_id = match auth {
        Ok(a) => a.id,
        Err(_) => {
            // 未登录 / 被封（403）/ token 过期：仅允许 ban 申诉，且必须通过验证码
            if body.kind != "ban" {
                return Err(DomainError::Unauthorized);
            }
            let username = body
                .username
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    DomainError::Validation("请填写被封的账户用户名".into())
                })?;
            captcha_check(&state, &body.captcha_id, &body.captcha_answer)
                .await?;
            let uid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE lower(username::text) = lower($1) AND status >= 2",
            )
            .bind(username)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let uid = uid.ok_or_else(|| {
                DomainError::Validation("未找到该用户名的封禁记录".into())
            })?;
            // 防刷：每用户名 3 次/小时（未结申诉上限 3 条之外的第二道闸）
            {
                let mut c = state.redis.clone();
                let k = format!("rl:appeal:{uid}");
                let n: i64 = c.incr(&k, 1).await.unwrap_or(0);
                if n == 1 {
                    let _: () = c.expire(&k, 3600).await.unwrap_or(());
                }
                if n > 3 {
                    return Err(DomainError::RateLimited);
                }
            }
            uid
        }
    };
    if !["hr", "warn", "ban", "other"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 仅支持 hr/warn/ban/other".into(),
        ));
    }
    if body.body.trim().len() < 10 {
        return Err(DomainError::Validation("申诉内容至少 10 字".into()));
    }
    // 未结申诉上限 3 条
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'",
    )
    .bind(user_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= 3 {
        return Err(DomainError::Validation(
            "未结申诉上限 3 条，请等待处理".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, ref_id, body) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(&body.kind)
    .bind(body.ref_id)
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct AppealRow {
    id: i64,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/appeals")]
pub async fn appeal_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<AppealRow> = sqlx::query_as(
        "SELECT id, kind, ref_id, body, status, result_note, created_at \
         FROM appeals WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealHandleReq {
    appeal_id: i64,
    accept: bool,
    note: String,
}

/// staff 侧申诉队列（open 优先，可按状态过滤）
#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminAppealRow {
    id: i64,
    username: String,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/appeals")]
pub async fn appeal_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AppealQueueQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::APPEAL_HANDLE,
    )
    .await?;
    let status = q.status.as_deref().unwrap_or("");
    let rows: Vec<AdminAppealRow> = sqlx::query_as(
        "SELECT a.id, u.username, a.kind, a.ref_id, a.body, a.status, a.result_note, a.created_at \
         FROM appeals a JOIN users u ON u.id = a.user_id \
         WHERE ($1 = '' OR a.status = $1) \
         ORDER BY (a.status = 'open') DESC, a.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealQueueQuery {
    status: Option<String>,
}

#[post("/admin/appeals/handle")]
pub async fn appeal_handle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealHandleReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::APPEAL_HANDLE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE appeals SET status = $2, handled_by = $1, handled_at = now(), result_note = $3 \
         WHERE id = $4 AND status = 'open'",
    )
    .bind(auth.id)
    .bind(if body.accept { "accepted" } else { "rejected" })
    .bind(body.note.trim())
    .bind(body.appeal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("申诉不存在或已处理".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "appeal.handle", Some(body.appeal_id))
        .await;
    Ok(ok(serde_json::json!({ "handled": body.appeal_id })))
}

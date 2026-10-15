//! 作弊申诉（用户侧，0369 M3）：一次性令牌申诉 / 提交 / 我的申诉。
//!
//! 与通用申诉（appeals.rs，hr/warn/ban/other）的关系：作弊申诉挂在案件上
//! （case_id），有 staff 复核结论（outcome）与 overturned 反向解除语义，
//! 故独立成域（staff 侧在 cheat_appeal_review.rs），共用 appeals 表。
//!
//! 三条提交路径：
//!   ① 未登录/被封用户 → 一次性令牌（ban 批准时随站内信发出，只存 sha3-256）
//!   ② 登录用户 → 提交针对自己某案件的申诉（case_id 校验归属）
//!   ③ 通用 /appeals 本体（kind='cheat' 亦走此处为 super set —— 本模块
//!      只做案件强关联的形态）

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 令牌摘要（sha3-256 十六进制）：库里只存摘要，明文只在通知里出现一次。
pub(crate) fn token_hash(raw: &str) -> String {
    use sha3::Digest;
    let mut h = sha3::Sha3_256::new();
    h.update(raw.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 未结作弊申诉上限（与通用申诉同口径）
const MAX_OPEN: i64 = 3;

#[derive(Deserialize)]
struct TokenAppealReq {
    token: String,
    body: String,
}

/// POST /appeals/token：停权/封禁用户凭一次性令牌提交申诉（免登录）。
/// 令牌形如 `{case_id}.{48 hex}`；核对摘要 + 有效期，**用后即焚**。
#[post("/appeals/token")]
pub async fn cheat_appeal_by_token(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TokenAppealReq>,
) -> DomainResult<HttpResponse> {
    let token = body.token.trim();
    let content = body.body.trim();
    if content.len() < 10 || content.len() > 4000 {
        return Err(DomainError::Validation(
            "申诉内容 10-4000 字符".into(),
        ));
    }
    let (cid, tail) = token
        .split_once('.')
        .ok_or_else(|| DomainError::Validation("令牌格式无效".into()))?;
    if tail.len() < 16 {
        return Err(DomainError::Validation("令牌格式无效".into()));
    }
    let case_id: i64 = cid
        .parse()
        .map_err(|_| DomainError::Validation("令牌格式无效".into()))?;
    let hash = token_hash(token);
    // 摘要匹配 + 未过期（一次性：消费后置 NULL，旧值立即失效）
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE appeal_token_hash = $1 \
           AND appeal_token_expires_at > now()",
    )
    .bind(&hash)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let uid = uid.ok_or_else(|| {
        DomainError::Validation("令牌无效或已过期（可能已被使用）".into())
    })?;
    // 案件必须属于该用户（防令牌跨用户使用）
    let owns: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reports \
         WHERE id = $1 AND ref_type = 'cheat' AND ref_id = $2)",
    )
    .bind(case_id)
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !owns {
        return Err(DomainError::Validation("令牌与案件不匹配".into()));
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= MAX_OPEN {
        return Err(DomainError::Validation(
            "未结申诉已达上限，请等待处理".into(),
        ));
    }
    // 建申诉（token_hash 留痕：哪枚令牌提交的）+ 用后即焚
    let mut tx = state.repo.db.begin().await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, case_id, body, token_hash) \
         VALUES ($1, 'cheat', $2, $3, $4) RETURNING id",
    )
    .bind(uid)
    .bind(case_id)
    .bind(content)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET appeal_token_hash = NULL, \
         appeal_token_expires_at = NULL WHERE id = $1",
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    notify_staff(&state, id, uid, case_id, true).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct CheatAppealReq {
    case_id: i64,
    body: String,
}

/// POST /cheat-appeals：登录用户对**自己**的案件提交作弊申诉。
#[post("/cheat-appeals")]
pub async fn cheat_appeal_submit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CheatAppealReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let content = body.body.trim();
    if content.len() < 10 || content.len() > 4000 {
        return Err(DomainError::Validation(
            "申诉内容 10-4000 字符".into(),
        ));
    }
    let owns: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reports \
         WHERE id = $1 AND ref_type = 'cheat' AND ref_id = $2)",
    )
    .bind(body.case_id)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !owns {
        return Err(DomainError::Validation("案件不存在或不属于你".into()));
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= MAX_OPEN {
        return Err(DomainError::Validation(
            "未结申诉已达上限，请等待处理".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, case_id, body) \
         VALUES ($1, 'cheat', $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.case_id)
    .bind(content)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    notify_staff(&state, id, auth.id, body.case_id, false).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MyCheatAppeal {
    id: i64,
    case_id: Option<i64>,
    body: String,
    status: String,
    outcome: Option<String>,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /my/cheat-appeals：我的作弊申诉（含结论与案件处置状态）。
#[get("/my/cheat-appeals")]
pub async fn cheat_appeal_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyCheatAppeal> = sqlx::query_as(
        "SELECT a.id, a.case_id, a.body, a.status, a.outcome, \
                a.result_note, a.created_at \
         FROM appeals a WHERE a.user_id = $1 AND a.kind = 'cheat' \
         ORDER BY a.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 新申诉 → 管理组信箱立项（署名取申请人；permission='cheater' 面）。
async fn notify_staff(
    state: &web::Data<std::sync::Arc<AppState>>,
    appeal_id: i64,
    uid: i64,
    case_id: i64,
    by_token: bool,
) {
    let uname: Option<String> =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten();
    let _ = sqlx::query(
        "INSERT INTO staffmessages (user_id, subject, body, permission) \
         VALUES ($1, $2, $3, 'cheater')",
    )
    .bind(uid)
    .bind(format!(
        "作弊申诉待复核（案件 #{case_id}，用户 {}）",
        uname.unwrap_or_else(|| format!("#{uid}"))
    ))
    .bind(format!(
        "申诉 #{appeal_id}（{}）指向案件 #{case_id}，请到「反作弊申诉」\
         队列复核（结论三选一：维持/减轻/推翻；推翻将反向解除该案处置）。",
        if by_token { "一次性令牌提交" } else { "登录提交" }
    ))
    .execute(&state.repo.db)
    .await;
}

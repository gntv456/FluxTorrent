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
use crate::repo::cheat_appeals as appeal_repo;
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
    if content.chars().count() < 10 || content.chars().count() > 4000 {
        return Err(DomainError::Validation("申诉内容 10-4000 字符".into()));
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
    let uid = appeal_repo::user_id_by_token_hash(&state.repo, &hash)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .ok_or_else(|| {
            DomainError::Validation("令牌无效或已过期（可能已被使用）".into())
        })?;
    // 案件必须属于该用户（防令牌跨用户使用）
    if !appeal_repo::case_owned_by_user(&state.repo, case_id, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    {
        return Err(DomainError::Validation("令牌与案件不匹配".into()));
    }
    let open = appeal_repo::open_appeal_count(&state.repo, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= MAX_OPEN {
        return Err(DomainError::Validation(
            "未结申诉已达上限，请等待处理".into(),
        ));
    }
    // 建申诉（token_hash 留痕：哪枚令牌提交的）+ 用后即焚（repo 内单事务）
    let id = appeal_repo::create_token_appeal(
        &state.repo,
        uid,
        case_id,
        content,
        &hash,
    )
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
    if content.chars().count() < 10 || content.chars().count() > 4000 {
        return Err(DomainError::Validation("申诉内容 10-4000 字符".into()));
    }
    let owns =
        appeal_repo::case_owned_by_user(&state.repo, body.case_id, auth.id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if !owns {
        return Err(DomainError::Validation("案件不存在或不属于你".into()));
    }
    let open = appeal_repo::open_appeal_count(&state.repo, auth.id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= MAX_OPEN {
        return Err(DomainError::Validation(
            "未结申诉已达上限，请等待处理".into(),
        ));
    }
    let id =
        appeal_repo::create_appeal(&state.repo, auth.id, body.case_id, content)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    notify_staff(&state, id, auth.id, body.case_id, false).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// GET /my/cheat-appeals：我的作弊申诉（含结论与案件处置状态）。
#[get("/my/cheat-appeals")]
pub async fn cheat_appeal_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = appeal_repo::my_appeals(&state.repo, auth.id)
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
    let uname = appeal_repo::username_of(&state.repo, uid).await;
    let _ =
        appeal_repo::insert_staffmessage(
            &state.repo,
            uid,
            format!(
                "作弊申诉待复核（案件 #{case_id}，用户 {}）",
                uname.unwrap_or_else(|| format!("#{uid}"))
            ),
            format!(
            "申诉 #{appeal_id}（{}）指向案件 #{case_id}，请到「反作弊申诉」\
             队列复核（结论三选一：维持/减轻/推翻；推翻将反向解除该案处置）。",
            if by_token { "一次性令牌提交" } else { "登录提交" }
        ),
        )
        .await;
}

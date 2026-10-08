//! 管理端凭证处置（0285）。
//!
//! 起因（本机实测）：旧版 `GET /admin/users/{id}` 只过 `staff()`，SELECT 里直接带
//! `u.passkey` 明文，且不写审计——class 92 论坛版主可取到站长 passkey，
//! 而 passkey 等于该用户的 tracker 身份（计流量、下载、做种全挂在其名下）。
//! 现在详情接口只回掩码，明文出口收归 `user.passkey.reveal`（默认仅主管/站长），
//! 两次出口都强制 outranks + 审计留痕（含理由）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

#[derive(Deserialize)]
struct PasskeyOpReq {
    user_id: i64,
    /// 操作理由（进审计）：查别人凭证必须说清为什么
    #[serde(default)]
    reason: String,
}

/// 统一的凭证处置前置：staff + 专项权限 + 严格高于目标 + 理由必填。
async fn gate(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    reason: &str,
) -> DomainResult<crate::http::AuthUser> {
    let auth = staff(req, state).await?;
    crate::authz::require_perm(
        state,
        &auth,
        crate::authz::perm::USER_PASSKEY_REVEAL,
    )
    .await?;
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    if reason.trim().is_empty() {
        return Err(DomainError::Validation(
            "请填写操作理由（将记入审计）".into(),
        ));
    }
    Ok(auth)
}

/// 查看 passkey 明文。默认拒绝，且每次查看都留痕。
#[post("/admin/users/passkey/reveal")]
pub async fn admin_passkey_reveal(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasskeyOpReq>,
) -> DomainResult<HttpResponse> {
    let uid = body.user_id;
    let auth = gate(&req, &state, uid, &body.reason).await?;
    let passkey: Option<String> = sqlx::query_scalar(
        "SELECT passkey FROM users WHERE id = $1 AND status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let passkey = passkey.ok_or(DomainError::NotFound(uid))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.passkey.reveal",
            Some(uid),
            Some(body.reason.trim()),
            None,
        )
        .await;
    tracing::warn!(
        actor = auth.id,
        target = uid,
        "管理端查看成员 passkey（已留审计）"
    );
    Ok(ok(
        serde_json::json!({ "user_id": uid, "passkey": passkey }),
    ))
}

/// 代为重置 passkey（passkey 泄露应急的标准动作）。
/// 旧 key 在 tracker 侧随 guard 版本轮询失效（≤3s，非「立即」——文案同口径）。
#[post("/admin/users/passkey/reset")]
pub async fn admin_passkey_reset(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasskeyOpReq>,
) -> DomainResult<HttpResponse> {
    let uid = body.user_id;
    let auth = gate(&req, &state, uid, &body.reason).await?;
    let target: Option<(String, i32)> = sqlx::query_as(
        "SELECT username, class_id FROM users WHERE id = $1 AND status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (username, _) = target.ok_or(DomainError::NotFound(uid))?;
    // 代为重置同样是处置动作：旧钥立刻失效（grace=false）
    let passkey = state.repo.update_passkey(uid, false).await?;
    crate::http::bump_guard_ver(&state).await;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.passkey.reset",
            Some(uid),
            Some(body.reason.trim()),
            None,
        )
        .await;
    // 被重置的人必须知道自己凭证被换过（否则客户端突然全部报「passkey 无效」只能靠猜）
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(uid)
    .bind("passkey 已由管理组重置")
    .bind(format!(
        "您好 {username}，管理组已为您的账号重置 passkey（原因：{}）。{} 旧下载任务中的 \
         announce 地址会失效，请到种子页重新下载 .torrent，或在客户端里把 announce 网址 \
         换成新的 passkey。如有疑问请回复本短信。",
        body.reason.trim(),
        "重置数秒内生效，"
    ))
    .execute(&state.repo.db)
    .await;
    tracing::warn!(actor = auth.id, target = uid, "管理端重置成员 passkey");
    Ok(ok(serde_json::json!({
        "user_id": uid,
        "username": username,
        "passkey": passkey,
    })))
}

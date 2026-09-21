//! 2FA 杂项面：管理员清除用户 2FA、盒子规则声明页、TGBot 绑定预留。
//! 从 twofa_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 管理员清除用户 2FA（丢失验证器救援通道）。
/// 场景：用户丢失验证器 App 且无恢复码 → 账号被 login_totp_check 永久锁死。
/// 权限：user.resetpass（与重置密码同档：身份核验后由管理员人工放行）+ ensure_outranks。
#[post("/admin/users/{id}/2fa/clear")]
pub(super) async fn admin_2fa_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_RESETPASS,
    )
    .await?;
    let uid = path.into_inner();
    // 等级护栏（与 admin_http 同口径）：操作者须严格高于目标用户
    {
        let db = &state.repo.db;
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let tc = target_class.ok_or(DomainError::NotFound(uid))?;
        if auth.class_id <= tc {
            return Err(DomainError::Forbidden);
        }
    }
    let updated = sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_enabled = FALSE \
         WHERE id = $1 AND (totp_enabled OR totp_secret IS NOT NULL)",
    )
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation("该用户未开启 2FA".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "2fa.admin_clear", Some(uid))
        .await;
    // 通知用户：2FA 已被管理员清除，下次登录仅需密码（建议尽快重新开启）
    let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES (NULL, $1, $2, $3)",
    )
    .bind(uid)
    .bind("两步验证已被管理员重置")
    .bind("你因丢失验证器申请救援，管理员已清除你的两步验证。下次登录仅需密码，请尽快在控制面板重新开启。")
    .execute(&state.repo.db)
    .await;
    Ok(ok(serde_json::json!({ "cleared": true, "user_id": uid })))
}

// ============ 盒子规则（声明页，NexusPHP box rules 口径） ============

#[get("/rules/box")]
pub(super) async fn box_rules() -> impl Responder {
    ok(serde_json::json!({
        "title": "盒子 / 独服 / 高速下载设备规则",
        "updated": "2026-09-09",
        "rules": [
            "本站允许盒子做种，但禁止「只下不做」的刷流量行为；H&R 规则对盒子同样生效。",
            "禁止利用盒子进行恶意下载打击他人做种（hit & run 连发）。",
            "同 IP 并发做种 ≥ 50 个种子视为盒子行为，请在个人中心登记；未登记不影响功能，但申诉时以登记为准。",
            "使用云服务器做种请注意流量费用，本站不对超额流量负责。",
            "违反以上规则的账号按 H&R 追责流程处理，可通过申诉系统复核。"
        ],
    }))
}

// ============ TGBot 通知通道预留（绑定 → worker 推送位） ============

#[derive(Deserialize)]
struct TgBindReq {
    chat_id: String,
}

/// 绑定 Telegram（预留：worker 侧推送促销/短讯通知的投递目标）
#[post("/me/tg-bind")]
pub(super) async fn tg_bind(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TgBindReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !body.chat_id.chars().all(|c| c.is_ascii_digit())
        || body.chat_id.len() < 5
    {
        return Err(DomainError::Validation(
            "Telegram chat_id 应为数字".into(),
        ));
    }
    sqlx::query("UPDATE users SET tg_chat_id = $2 WHERE id = $1")
        .bind(auth.id)
        .bind(&body.chat_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "tg.bind", None).await;
    Ok(ok(serde_json::json!({ "bound": body.chat_id })))
}

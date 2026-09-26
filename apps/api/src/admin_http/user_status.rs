use actix_web::{post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 下载权限 / 挂起 开关（tracker announce 执行点生效）
#[derive(Deserialize)]
struct UserFlagsReq {
    user_id: i64,
    #[serde(default)]
    download_enabled: Option<bool>,
    #[serde(default)]
    suspended: Option<bool>,
}

#[put("/admin/users/flags")]
async fn user_flags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserFlagsReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_FLAGS)
        .await?;
    if body.download_enabled.is_none() && body.suspended.is_none() {
        return Err(DomainError::Validation("至少提供一个开关".into()));
    }
    // 挂起 / 禁下载属伤害性操作，须严格高于目标等级
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let n = sqlx::query(
        "UPDATE users SET \
            download_enabled = COALESCE($2, download_enabled), \
            suspended = COALESCE($3, suspended) \
         WHERE id = $1",
    )
    .bind(body.user_id)
    .bind(body.download_enabled)
    .bind(body.suspended)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "user.flags", Some(body.user_id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!(
            "开关变更 download_enabled={:?} suspended={:?}",
            body.download_enabled, body.suspended
        ),
    )
    .await;
    // 挂起/禁下载变更需立即作用于 tracker（否则 passkey 缓存 60s 内仍有效）
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "download_enabled": body.download_enabled,
        "suspended": body.suspended,
    })))
}

#[derive(Deserialize)]
pub(super) struct SearchQ {
    #[serde(default = "empty_q")]
    pub(super) q: String,
}
fn empty_q() -> String {
    String::new()
}

#[derive(Deserialize)]
struct SetStatusReq {
    user_id: i64,
    /// 0 正常 1 禁言 2 封禁
    status: i16,
    /// 操作理由（前端必填项；此前被 serde 静默丢弃）
    #[serde(default)]
    reason: String,
}

#[post("/admin/users/status")]
async fn user_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetStatusReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_STATUS)
        .await?;
    if !(0..=2).contains(&body.status) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    // 操作者等级须严格大于目标用户（防同级/下级操作上级）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let n = sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
        .bind(body.user_id)
        .bind(body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在".into()));
    }
    // 审计修复（P2）：封禁/禁言须即刻失效 tracker 的 passkey 缓存（≤60s 窗口内被 ban 用户
    // 仍可 announce）。flags 端点早有同款调用，此处此前遗漏。
    if body.status >= 1 {
        crate::http::bump_guard_ver(&state).await;
    }
    // api 侧用户状态短缓存（5s TTL）同步失效：封禁/恢复立即生效
    state.user_status_cache.invalidate(body.user_id);
    state
        .repo
        .audit(Some(auth.id), "user.set_status", Some(body.user_id))
        .await;
    let label = match body.status {
        1 => "禁言",
        2 => "封禁",
        _ => "恢复正常",
    };
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &if body.reason.trim().is_empty() {
            format!("状态 → {}", body.status)
        } else {
            format!("{label}：{}", body.reason.trim())
        },
    )
    .await;
    // 0209 P1-9：状态变更通知当事人（此前理由只进内部 modify_log，
    // 用户只能从公开 ban-log 猜状态）。封禁走邮件（已无法登录看 PM）；
    // 禁言/恢复走 PM 必达 + 邮件尽力。
    {
        let target: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT username, email FROM users WHERE id = $1",
        )
        .bind(body.user_id)
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten();
        if let Some((username, email)) = target {
            let site: String = sqlx::query_scalar(
                "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'site_name'), 'FluxTorrent')",
            )
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_else(|_| "FluxTorrent".into());
            let reason_txt = if body.reason.trim().is_empty() {
                "（未填写）".to_string()
            } else {
                body.reason.trim().to_string()
            };
            let subject = format!("[{site}] 账号状态变更：{label}");
            let body_text = match body.status {
                2 => format!(
                    "你好 {username}，你的账号已被{label}。\n理由：{reason_txt}\n如认为有误，可通过站点的申诉通道提交申诉。",
                ),
                1 => format!(
                    "你好 {username}，你的账号已被{label}（发言受限，其余功能正常）。\n理由：{reason_txt}",
                ),
                _ => format!(
                    "你好 {username}，你的账号状态已恢复正常。此前的限制已解除。",
                ),
            };
            // 封禁：PM 也写（解封后可见历史）+ 邮件必发；其余：notify 双通道
            if body.status == 2 {
                let _ = crate::mailer::site_message(
                    &state.repo.db,
                    body.user_id,
                    &subject,
                    &body_text,
                )
                .await;
                if let Some(to) = email {
                    let cfg = crate::mailer::smtp_config(&state.repo.db).await;
                    let (s, b) = (subject.clone(), body_text.clone());
                    actix_web::rt::spawn(async move {
                        if let Some(cfg) = cfg {
                            let _ = crate::gaps_http::send_generic_mail(
                                &cfg.url, &cfg.from, &to, &s, &b,
                            )
                            .await;
                        }
                    });
                }
            } else {
                crate::mailer::notify(
                    &state.repo.db,
                    body.user_id,
                    email,
                    &subject,
                    &body_text,
                )
                .await;
            }
        }
    }
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "status": body.status }),
    ))
}

#[derive(Deserialize)]
struct SetClassReq {
    user_id: i64,
    class_id: i32,
}

#[post("/admin/users/class")]
async fn user_set_class(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetClassReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 站长（99）才能调整等级；且禁止操作同级/更高级账户
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CLASS)
        .await?;
    // 不能把他人设为站长（与角色等级无关的结构性约束）
    if body.class_id >= 99 {
        return Err(DomainError::Forbidden);
    }
    // 审计修复（P0 越权）：此前允许把下级提到高于操作者自己的等级（93 档可授 98 档），
    // 改完后自己反被该用户压制。封顶 = 操作者等级 - 1，与 ensure_outranks 口径一致。
    if body.class_id >= auth.class_id {
        return Err(DomainError::Validation(format!(
            "不能把用户提升到不低于自己的等级（{} ≥ {}）",
            body.class_id, auth.class_id
        )));
    }
    // 与目标同高或更低不可调整（站长改自己也应被拦）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let n = sqlx::query(
        "UPDATE users SET class_id = $2 WHERE id = $1 AND class_id < 99",
    )
    .bind(body.user_id)
    .bind(body.class_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在或不可调整".into()));
    }
    // api 侧用户状态短缓存同步失效：等级变更立即影响权限判定
    state.user_status_cache.invalidate(body.user_id);
    state
        .repo
        .audit(Some(auth.id), "user.set_class", Some(body.user_id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("等级 → {}", body.class_id),
    )
    .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "class_id": body.class_id }),
    ))
}

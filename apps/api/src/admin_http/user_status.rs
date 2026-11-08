use actix_web::{post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_class_exists, ensure_outranks, staff};

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
    // 0482 审计前后值：变更前快照（变更点只落 body 里的新值）
    let before: Option<(bool, bool)> = sqlx::query_as(
        "SELECT download_enabled, suspended FROM users WHERE id = $1",
    )
    .bind(body.user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "UPDATE users SET \
            download_enabled = COALESCE($2, download_enabled), \
            download_locked_by = CASE \
                WHEN $2 IS NULL THEN download_locked_by \
                WHEN $2 THEN NULL \
                ELSE 'staff' END, \
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
    let (b_dl, b_susp) = before.unwrap_or((true, false));
    let after = serde_json::json!({
        "before": { "no_dl": !b_dl, "suspended": b_susp },
        "after": { "no_dl": !body.download_enabled.unwrap_or(b_dl),
                   "suspended": body.suspended.unwrap_or(b_susp) },
    });
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.flags",
            Some(body.user_id),
            None,
            Some(after),
        )
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
    // 理由上限 200 字（P-157 红线：4000 字的 reason 原样进了留痕表，还会被写进
    // 给当事人的 PM 正文与运营 webhook；同仓 `audit_detail` 是按 200 字截断的）
    if body.reason.chars().count() > 200 {
        return Err(DomainError::Validation("理由需 ≤200 字符".into()));
    }
    // 操作者等级须严格大于目标用户（防同级/下级操作上级）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    // 0482 审计前后值：变更前状态快照
    let before_status: Option<i16> =
        sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
            .bind(body.user_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 禁言必须落到论坛侧真正读的那面旗子：`users.forumpost`。
    // 旧版只写 status=1，而鉴权层只拦 status>=2、论坛/漂流瓶只看 forumpost
    // （且全仓没有任何写口）⇒ 点「禁言」后用户照样发帖回帖刷漂流瓶。
    let n = sqlx::query(
        "UPDATE users SET status = $2, forumpost = ($2 <> 1) WHERE id = $1",
    )
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
    // 三轮遗留（2026-10-07）：封禁同步回收其未用邀请码（置 status=2 作废）
    // ——信任链在封禁动作上不能断开：被封者名下未过期码仍可注册小号。
    if body.status == 2 {
        let revoked = sqlx::query(
            "UPDATE invites SET status = 2              WHERE inviter_id = $1 AND status = 0 AND expires_at > now()",
        )
        .bind(body.user_id)
        .execute(&state.repo.db)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
        if revoked > 0 {
            tracing::info!(
                uid = body.user_id,
                revoked,
                "封禁联动作废未用邀请码"
            );
        }
    }
    // api 侧用户状态短缓存（5s TTL）同步失效：封禁/恢复立即生效
    state.user_status_cache.invalidate(body.user_id);
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.set_status",
            Some(body.user_id),
            Some(&body.reason),
            Some(serde_json::json!({
                "before": before_status,
                "after": body.status,
            })),
        )
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
    // 0209 P1-9：状态变更通知当事人（渲染+投递段拆至 user_status_notify.rs）
    crate::admin_http::notify_status_change(
        &state,
        body.user_id,
        body.status,
        &label,
        body.reason.trim(),
    )
    .await;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CLASS)
        .await?;
    // 档位号必须真实存在（实测 class_id=90 撞 FK ⇒ 500，批四十三 P-171）
    ensure_class_exists(&state.repo.db, body.class_id).await?;
    // 不能把他人设为站长（与角色等级无关的结构性约束）
    if body.class_id >= 99 {
        return Err(DomainError::Forbidden);
    }
    // 审计修复（P0 越权）：封顶 = 操作者等级 - 1（旧版 93 档可授 98 档，改完反被压制）
    if body.class_id >= auth.class_id {
        return Err(DomainError::Validation(format!(
            "不能把用户提升到不低于自己的等级（{} ≥ {}）",
            body.class_id, auth.class_id
        )));
    }
    // 与目标同高或更低不可调整（站长改自己也应被拦）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    // 0482 审计前后值：变更前等级快照
    let before_class: Option<i32> =
        sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
            .bind(body.user_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
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
        .audit_detail(
            Some(auth.id),
            "user.set_class",
            Some(body.user_id),
            None,
            Some(serde_json::json!({
                "before": before_class,
                "after": body.class_id,
            })),
        )
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

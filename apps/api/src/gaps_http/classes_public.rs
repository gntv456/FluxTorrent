//! P2 触点补齐（0226）：等级要求公开页 + 自助解封（首次宽恕）。
//!
//! · `/classes`：UNIT3D `/groups/requirements` 口径——等级门槛与特权对全站
//!   用户公开透明（触点 #2「了解门槛」）。数据单源 class_rules/user_classes，
//!   只读；登录即可看（不泄敏感，规则页本就该公开）。
//! · `POST /auth/self-unban`：被禁用户冷却期内一次自助解封（触点 #24，
//!   对齐 NexusPHP 1.10.2 self-service unsealing）。前提 self_unban_cooldown_hours
//!   > 0（站长启用）；用掉一次记 users.self_unban_used，audit 留痕。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde_json::json;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 等级要求公开页（登录态；只读聚合）
#[get("/classes")]
pub(super) async fn classes_public(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 成长线（0-12）+ 门槛四维；职务级（90+）不列门槛、只列名称
    let rows: Vec<(i32, String, i64, i64, i64, i64, bool)> = sqlx::query_as(
        "SELECT uc.id, uc.name, \
                    COALESCE(cr.min_uploaded, 0)::bigint, \
                    COALESCE(cr.min_download_count, 0)::bigint, \
                    COALESCE(cr.min_seed_hours, 0)::bigint, \
                    COALESCE(cr.min_account_age_days, 0)::bigint, \
                    COALESCE(cr.demotable, false) \
             FROM user_classes uc \
             LEFT JOIN class_rules cr ON cr.class_id = uc.id \
             WHERE uc.id < 90 \
             ORDER BY uc.id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let staff: Vec<(i32, String)> = sqlx::query_as(
        "SELECT id, name FROM user_classes WHERE id >= 90 ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let me_class: i32 =
        sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // E3：本人四维现状（与 worker class_auto_adjust 的 stats 同一口径），
    // 前端算「距下一级还差多少」。账龄按日截断，与 EXTRACT(DAY) 升级判定一致。
    let me_stats = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT u.uploaded::bigint, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id \
                  AND s.completed_at IS NOT NULL)::bigint, \
                (SELECT COALESCE(sum(s.seeded_seconds),0)/3600 FROM snatches s \
                  WHERE s.user_id = u.id)::bigint, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|(up, dl, sh, age)| {
        json!({
            "uploaded": up, "download_count": dl,
            "seed_hours": sh, "account_age_days": age,
        })
    });
    Ok(ok(json!({
        "me_class": me_class,
        "me_stats": me_stats,
        "classes": rows.iter().map(|(id, name, up, dl, sh, age, demo)| json!({
            "id": id, "name": name,
            "min_uploaded": up, "min_download_count": dl,
            "min_seed_hours": sh, "min_account_age_days": age,
            "demotable": demo,
        })).collect::<Vec<_>>(),
        "staff_classes": staff.iter().map(|(id, name)| json!({
            "id": id, "name": name,
        })).collect::<Vec<_>>(),
    })))
}

/// 自助解封（首次宽恕）：被禁用户本人调用。
#[post("/auth/self-unban")]
pub(super) async fn self_unban(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    // 注意：require_auth 对被禁用户是否放行取决于闸门口径——被禁(status=2)用户
    // 的 token 仍有效时走此端点自救；若已被强制登出则需重新登录（登录口对被禁
    // 账号放行到本端点由 setup_gate/模块门之外的业务闸门决定，此处不重复设卡）。
    let auth = require_auth(&req, &state).await?;
    let cooldown: f64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'self_unban_cooldown_hours')::float8, 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0.0);
    if cooldown <= 0.0 {
        return Err(DomainError::Validation(
            "本站未开放自助解封，请联系管理组申诉".into(),
        ));
    }
    let (status, used): (i16, bool) = sqlx::query_as(
        "SELECT status, self_unban_used FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if status != 2 {
        return Err(DomainError::Validation("账号未被封禁，无需解封".into()));
    }
    if used {
        return Err(DomainError::Validation(
            "自助解封机会已用完（每人一次），请走申诉通道".into(),
        ));
    }
    // 解封：status 2→0（正常，0001 口径 0=正常 1=禁言 2=封号），标记已用；
    // 留痕走 audit（status 变更与 user.set_status 同口径可查）
    let n = sqlx::query(
        "UPDATE users SET status = 0, self_unban_used = true \
         WHERE id = $1 AND status = 2",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "解封失败：账号状态已变化，请刷新重试".into(),
        ));
    }
    state.user_status_cache.invalidate(auth.id);
    state
        .repo
        .audit(Some(auth.id), "self_unban", Some(auth.id))
        .await;
    Ok(ok(json!({ "unbanned": true })))
}

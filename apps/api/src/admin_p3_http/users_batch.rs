//! P2-9 用户批量操作
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{check_ids, modify_log, staff};

// ============ P2-9 用户批量操作 ============

#[derive(Deserialize)]
struct UserBatchReq {
    /// status（value 0/1/2）| class（value 等级数字）
    action: String,
    ids: Vec<i64>,
    value: i32,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/users/batch")]
async fn admin_users_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let perm = match body.action.as_str() {
        "status" => crate::authz::perm::USER_STATUS,
        "class" => crate::authz::perm::USER_CLASS,
        _ => return Err(DomainError::Validation("未知批量动作".into())),
    };
    crate::authz::require_perm(&state, &auth, perm).await?;
    check_ids(&body.ids)?;
    if body.action == "status" && !(0..=2).contains(&body.value) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    if body.action == "class" && !(1..=98).contains(&body.value) {
        return Err(DomainError::Validation(
            "等级取值 1-98（站长除外）".into(),
        ));
    }
    // 审计修复（P0 越权，与 user_set_class 同病）：批量提级同样不得越过操作者自己，
    // 否则 93 档管理员可批量把下级提到 98、改完反被压制。
    if body.action == "class" && body.value >= auth.class_id {
        return Err(DomainError::Validation(format!(
            "批量等级不能不低于自己（{} ≥ {}）",
            body.value, auth.class_id
        )));
    }
    let db = &state.repo.db;
    let mut updated: u64 = 0;
    let mut skipped: Vec<i64> = Vec::new();
    for uid in &body.ids {
        // 越权防护：只能操作严格低于自己等级的用户
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(tc) = target_class else {
            skipped.push(*uid);
            continue;
        };
        if tc >= auth.class_id {
            skipped.push(*uid);
            continue;
        }
        let n = match body.action.as_str() {
            "status" => {
                sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
                    .bind(uid)
                    .bind(body.value as i16)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .rows_affected()
            }
            "class" => {
                sqlx::query("UPDATE users SET class_id = $2 WHERE id = $1")
                    .bind(uid)
                    .bind(body.value)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .rows_affected()
            }
            _ => 0,
        };
        if n > 0 {
            updated += n;
            // 批量封禁同步失效 tracker passkey 缓存（与单用户 user_set_status 同口径；
            // 旧版漏掉导致被批量封禁用户最长 60s 仍可 announce）
            if body.action == "status" && body.value >= 1 {
                crate::http::bump_guard_ver(&state).await;
            }
            // api 侧用户状态短缓存（5s TTL）同步失效
            state.user_status_cache.invalidate(*uid);
            let content = match body.action.as_str() {
                "status" => format!(
                    "批量状态 → {}{}",
                    body.value,
                    body.reason
                        .as_deref()
                        .map(|r| format!("（{r}）"))
                        .unwrap_or_default()
                ),
                "class" => format!("批量等级 → {}", body.value),
                _ => String::new(),
            };
            modify_log(db, *uid, Some(auth.id), &content).await;
            state
                .repo
                .audit(
                    Some(auth.id),
                    &format!("user.batch.{}", body.action),
                    Some(*uid),
                )
                .await;
        }
    }
    Ok(ok(
        serde_json::json!({ "updated": updated as i64, "skipped": skipped }),
    ))
}

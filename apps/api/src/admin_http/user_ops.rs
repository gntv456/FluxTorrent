use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 「修改上传量等」（参考站用户详情按钮口径）：delta 语义，正加负减，下限 0
#[derive(Deserialize)]
struct UserAdjustReq {
    user_id: i64,
    #[serde(default)]
    uploaded_delta: Option<i64>,
    #[serde(default)]
    downloaded_delta: Option<i64>,
    #[serde(default)]
    spark_delta: Option<i64>,
    /// 增发邀请码数量（正数为增发，直接发放有效邀请）
    #[serde(default)]
    invite_grant: Option<i32>,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/users/adjust")]
async fn user_adjust(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserAdjustReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if body.uploaded_delta.is_none()
        && body.downloaded_delta.is_none()
        && body.spark_delta.is_none()
        && body.invite_grant.is_none()
    {
        return Err(DomainError::Validation("至少提供一项调整".into()));
    }
    // 数值调整仅 sysop/管理员（等级 93+）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    // 数值调整同属伤害性操作，须严格高于目标等级
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT uploaded, downloaded, spark_balance FROM users WHERE id = $1",
    )
    .bind(body.user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up0, down0, spark0) =
        row.ok_or(DomainError::NotFound(body.user_id))?;
    let up = ((up0 as i128 + body.uploaded_delta.unwrap_or(0) as i128).max(0))
        as i64;
    let down = ((down0 as i128 + body.downloaded_delta.unwrap_or(0) as i128)
        .max(0)) as i64;
    let spark = ((spark0 as i128 + body.spark_delta.unwrap_or(0) as i128)
        .max(0)) as i64;
    sqlx::query(
        "UPDATE users SET uploaded = $2, downloaded = $3, spark_balance = $4 WHERE id = $1",
    )
    .bind(body.user_id)
    .bind(up)
    .bind(down)
    .bind(spark)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 火花调整写流水（余额权威在 spark_ledger；kind=admin，操作者入 ref_id）
    if let Some(delta) = body.spark_delta {
        if delta != 0 {
            // 审计修复（P0 错账）：流水必须记「实际发生的差额」而非请求 delta——
            // 余额不足扣成负数时被 clamp 到 0，此前流水仍记请求值，
            // sum(ledger) 与 balance 永久撕裂（对照 amountbonus / increment-bulk 已修口径）。
            let actual_delta = spark - spark0;
            if actual_delta != 0 {
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'adjust', $3, $4, $5)",
                )
                .bind(body.user_id)
                .bind(actual_delta)
                .bind(auth.id)
                .bind(format!(
                    "admin-adjust-{}-{}",
                    body.user_id,
                    uuid::Uuid::new_v4().simple()
                ))
                .bind(spark)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    // 增发邀请：直接生成有效邀请码（30 天有效，NP takeinvite 口径）
    let mut granted_codes: Vec<String> = Vec::new();
    if let Some(grant) = body.invite_grant {
        if grant > 0 {
            for _ in 0..grant.min(50) {
                let code = uuid::Uuid::new_v4().simple().to_string();
                sqlx::query(
                    "INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, now() + interval '30 days')",
                )
                .bind(body.user_id)
                .bind(&code)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                granted_codes.push(code);
            }
        } else if grant < 0 {
            // 负数：回收最早到期的未用邀请
            sqlx::query(
                "DELETE FROM invites WHERE ctid IN (\
                    SELECT ctid FROM invites WHERE inviter_id = $1 AND status = 0 \
                    ORDER BY expires_at LIMIT $2)",
            )
            .bind(body.user_id)
            .bind((-grant) as i64)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    state
        .repo
        .audit(Some(auth.id), "user.adjust", Some(body.user_id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!(
            "数值调整 up{:+}/down{:+}/spark{:+}/invite{:+}",
            body.uploaded_delta.unwrap_or(0),
            body.downloaded_delta.unwrap_or(0),
            body.spark_delta.unwrap_or(0),
            body.invite_grant.unwrap_or(0)
        ),
    )
    .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "uploaded": up, "downloaded": down, "spark": spark,
        "invite_grant": body.invite_grant.unwrap_or(0),
        "granted_codes": granted_codes,
        "note": body.note,
    })))
}

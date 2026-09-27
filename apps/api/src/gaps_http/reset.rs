//! 找回密码端点（0020）：POST /auth/password/forgot + reset。
//! 从 gaps_http/mail.rs 按域拆出（SMTP/发信助手留在 mail.rs）。

use actix_web::{post, web, HttpResponse};
use redis::AsyncCommands;
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::login_aux::hash_token;
use super::mail::send_reset_mail;

#[post("/auth/password/forgot")]
pub async fn password_forgot(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ForgotReq>,
) -> DomainResult<HttpResponse> {
    // 限流：按邮箱 3 次/小时
    let mut c = state.redis.clone();
    let key = format!("rl:pwdforgot:{}", body.email.to_lowercase());
    let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = AsyncCommands::expire(&mut c, &key, 3600)
            .await
            .unwrap_or(());
    }
    if n > 3 {
        return Err(DomainError::RateLimited);
    }

    let uid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
            .bind(body.email.trim().to_lowercase())
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    // 无论命中与否都返回相同响应（防账号枚举）
    if let Some(uid) = uid {
        let token = format!("pwd_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO password_resets (user_id, token_hash, expires_at) \
             VALUES ($1, $2, now() + interval '30 minutes')",
        )
        .bind(uid)
        .bind(hash_token(&token))
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(uid), "pwd.forgot", None).await;

        // 0208 P0：SMTP 配置「站点设定优先，env 兜底」——后台填好 smtp_* 即生效
        let cfg = crate::mailer::smtp_config(&state.repo.db).await;
        if cfg.is_none() {
            // 审计修复（P1 凭据泄露面）：重置 token 明文进日志任何可读日志的人都能接管
            // 账号。生产（FLUX_DEV≠1）直接拒绝服务并要求配置 SMTP；开发态保留日志闭环。
            if std::env::var("FLUX_DEV").unwrap_or_default() != "1" {
                tracing::error!(
                    uid,
                    "SMTP 未配置且非开发态：拒绝生成密码重置 token（防凭据经日志泄露）"
                );
                return Err(DomainError::Internal(anyhow::anyhow!(
                    "SMTP 未配置：生产环境禁止以日志方式暴露重置 token"
                )));
            }
            tracing::warn!(%token, "SMTP 未配置：重置 token 输出到日志（仅限开发态闭环）");
        } else {
            let cfg = cfg.unwrap();
            // 真实投递（lettre）：URL = smtps://user:pass@host:port 或 smtp://host:port；
            // 后台线程发送，失败仅记日志不影响响应。
            let base = std::env::var("PUBLIC_API_URL")
                .unwrap_or_else(|_| "http://localhost:3000".into());
            let link = format!("{base}/reset?token={token}");
            let email_addr = body.email.trim().to_lowercase();
            let site = sqlx::query_scalar::<_, String>(
                "SELECT value FROM site_settings WHERE name = 'site_name'",
            )
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "FluxTorrent".into());
            actix_web::rt::spawn(async move {
                match send_reset_mail(
                    &cfg.url,
                    &cfg.from,
                    &email_addr,
                    &site,
                    &link,
                )
                .await
                {
                    Ok(_) => tracing::info!(
                        uid,
                        "password reset mail sent to {email_addr}"
                    ),
                    Err(e) => {
                        tracing::error!(uid, "password reset mail failed: {e}")
                    }
                }
            });
        }
    }
    Ok(ok(serde_json::json!({
        "message": "如邮箱存在，重置链接已发送（30 分钟有效）"
    })))
}

#[derive(Deserialize)]
struct ResetReq {
    token: String,
    new_password: String,
}

#[post("/auth/password/reset")]
pub async fn password_reset(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetReq>,
) -> DomainResult<HttpResponse> {
    if body.new_password.len() < 8 {
        return Err(DomainError::Validation("密码至少 8 位".into()));
    }
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM password_resets \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(hash_token(body.token.trim()))
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::Validation("token 无效或已过期".into()));
    };
    let hash = crate::domain::hash_password(&body.new_password)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET pass_hash = $2 WHERE id = $1")
        .bind(uid)
        .bind(&hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE password_resets SET used_at = now() WHERE token_hash = $1",
    )
    .bind(hash_token(body.token.trim()))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 撤销该用户全部会话（改密后旧 token 全失效）。
    // 审计修复（P1）：此前只写 Redis nbf —— Redis 重启/故障后 24h 内旧 JWT「复活」
    // （JWT 与密码无关）。补写 DB 权威表 token_revocations（与 logout/改密路径同口径）。
    sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, EXTRACT(EPOCH FROM now())::bigint) \
         ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut c = state.redis.clone();
    let _: () = AsyncCommands::set_ex(
        &mut c,
        format!("logout_nbf:{uid}"),
        chrono::Utc::now().timestamp(),
        86400u64,
    )
    .await
    .unwrap_or(());
    state.repo.audit(Some(uid), "pwd.reset", None).await;
    Ok(ok(serde_json::json!({ "reset": true })))
}

#[derive(Deserialize)]
struct ForgotReq {
    email: String,
}

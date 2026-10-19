//! 找回密码端点（0020）：POST /auth/password/forgot + reset。
//! 从 gaps_http/mail.rs 按域拆出（SMTP/发信助手留在 mail.rs）。

use actix_web::{post, web, HttpRequest, HttpResponse};
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
    // P-73：这是四条匿名计数里最贵的一条——旧写法的 `unwrap_or(0)` 在 Redis
    // 抖动时把闸门拆掉，重置邮件可同时铸 6 条流水并连发垃圾信；而
    // `if n == 1 { expire }` 一失败就留下永不过期的键 ⇒ 这个邮箱此后**永远**
    // 申请不到找回密码（真正的账号恢复拒绝服务，且只能人工删键）。
    // rate_window 两者都堵：fail-close，窗口进键名（按整点对齐，跨窗最坏
    // 2×3 的尖峰是所有固定窗口的通性，与申诉档同口径）。
    crate::http::rate_window(
        &state,
        &format!("pwdforgot:{}", body.email.to_lowercase()),
        3,
        3600,
    )
    .await?;

    // P-102/P-104 能力前置：SMTP 或站点基址任一缺失，就在这里如实回
    // 503/4103——旧写法是「先铸令牌 → 拼不出链接 → 抛 500 内部错误」，
    // 结果是库里每次尝试多留一行无效 token，而用户和运维都看不出真正原因
    // （实测现网 SMTP 已配好仍一律 500，因为基址读的是 PUBLIC_API_URL）。
    // 该判据与「邮箱是否存在」无关，不给账号枚举开口子。
    let cfg = crate::mailer::smtp_config(&state.repo.db).await;
    let base = crate::mailer::site_base();
    let dev = std::env::var("FLUX_DEV").unwrap_or_default() == "1";
    if base.is_none() || (cfg.is_none() && !dev) {
        tracing::error!(
            base_set = base.is_some(),
            smtp_set = cfg.is_some(),
            "password reset refused: 邮件能力不完整（站点基址/SMTP）"
        );
        return Err(DomainError::MailUnconfigured);
    }
    // 开发态缺 SMTP 时的日志闭环沿用旧行为（token 只进日志，不入库外流）
    let base = base.unwrap_or_default();

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

        let Some(cfg) = cfg else {
            // 仅开发态可达（生产已在上面拒）：重置 token 明文进日志，闭环用
            tracing::warn!(%token, "SMTP 未配置：重置 token 输出到日志（仅限开发态闭环）");
            return Ok(ok(serde_json::json!({
                "message": "如邮箱存在，重置链接已发送（30 分钟有效）"
            })));
        };
        // 真实投递（lettre）：基址走 mailer::site_base() 的统一口径
        // （P-100：站内页面链接不能用 API 域名，那样点开后必然 404）。
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetReq>,
) -> DomainResult<HttpResponse> {
    // P-105 同族收口：这是全站唯一没有匿名限流的认证仪式端点——
    // passkey-login begin/finish、passkeys finish、password/forgot 都上了
    // rate_window，唯独留下它做无限次的令牌试错预言机（错码与「已用/过期」
    // 可分辨）。uuid4 猜不中不等于可以不限：撞库与打库的门槛都在这条口上。
    crate::http::rate_window(
        &state,
        &format!("pwdreset:{}", crate::http::client_ip(&req)),
        10,
        60,
    )
    .await?;
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
    // P-410：重置成功就是「用户已经拿到并设置了新密码」——`must_reset_password`
    // 必须一起清掉。旧写法只改 pass_hash，而自助改密口（`me_security.rs`）会清：
    // 于是拿装机临时密码、走「忘记密码」的用户，改完密码登录回来仍被回显
    // `must_reset_password=True`，安装向导下一步继续吃 400（同一个标志位，
    // 两条出口两把尺）。与改密同口径：连同 5s 状态缓存一起失效。
    sqlx::query(
        "UPDATE users SET pass_hash = $2, must_reset_password = false \
         WHERE id = $1",
    )
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
    // 与改密同口径：`must_reset_password` 是 5s 状态缓存的字段之一，不清缓存
    // 的话重置完立刻走下一步仍可能被旧值卡住（me_security.rs 同处注释）。
    state.user_status_cache.invalidate(uid);
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

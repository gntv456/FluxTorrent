//! C7-#4：注册后邮件激活通道（真实发信版）。
//!
//! 链路：注册（email_verify 模式）→ 建号（email_verified_at=NULL）→ 发验证信
//! （token 明文只在邮件，SHA3-256 落库，48h 过期）→ 未验证登录被拦（4103）
//! → GET /auth/email/verify?token=... 激活 → 登录放行。
//! 重发：POST /auth/email/resend（登录被拦也会走到这里拿 token 重发）。
//! 幂等/限流：同 user 未过期 token 只有一枚有效（重发作废旧枚）；
//! resend 按 user 60s 一次（throttle 键）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, require_auth, throttle};
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

pub(super) const TOKEN_TTL_HOURS: i64 = 48;

/// 生成明文 token（64 hex）与 SHA3-256 哈希。明文只进邮件不落库。
fn mint_token() -> (String, String) {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let plain: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    let hash = sha3_256_hex(&plain);
    (plain, hash)
}

fn sha3_256_hex(s: &str) -> String {
    use sha3::{Digest, Sha3_256};
    let mut h = Sha3_256::new();
    h.update(s.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 站点当前是否 email_verify 模式（未验证拦截只在此时生效）。
pub(super) async fn mode_is_email_verify(db: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'registration_mode'), '') = 'email_verify'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

/// 注册尾部调用：建号成功后发验证信（email_verify 模式才走）。
/// 发信失败不回滚账号（用户可用 resend 补发）——降级为日志。
pub(super) async fn send_verification(
    state: &Db,
    user_id: i64,
    email: &str,
    username: &str,
) {
    if let Err(e) =
        send_verification_inner(state, user_id, email, username).await
    {
        tracing::warn!(user_id, error = %e, "验证信发送失败（可 resend 补发）");
    }
}

async fn send_verification_inner(
    state: &Db,
    user_id: i64,
    email: &str,
    username: &str,
) -> anyhow::Result<()> {
    let (plain, hash) = mint_token();
    // 旧未过期枚全部作废（同 user 同刻只有一枚有效）
    sqlx::query(
        "UPDATE email_verifications SET expires_at = now() - interval '1s' \
         WHERE user_id = $1 AND verified_at IS NULL AND expires_at > now()",
    )
    .bind(user_id)
    .execute(&state.repo.db)
    .await?;
    sqlx::query(
        "INSERT INTO email_verifications (user_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + ($3::bigint * interval '1 hour'))",
    )
    .bind(user_id)
    .bind(&hash)
    .bind(TOKEN_TTL_HOURS)
    .execute(&state.repo.db)
    .await?;
    let site: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'site_name'), 'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let body = format!(
        "你好 {username}，欢迎注册 {site}。\n\n\
         请在 48 小时内点击以下链接完成邮箱激活（激活后才能登录）：\n\
         /auth/email/verify?token={plain}\n\n\
         如果这不是你的操作，请忽略本邮件。"
    );
    crate::mailer::send_mail_with(
        &state.repo.db,
        email,
        &format!("【{site}】邮箱激活"),
        &body,
    )
    .await
}

/// 登录拦截点（login 调用）：email_verify 模式且该用户未激活 → Some。
/// 返回 Some 时登录终止（Validation 带标记前缀，login 转译成 4103 信封）。
pub(super) async fn guard_login(
    state: &Db,
    user_id: i64,
) -> Option<DomainError> {
    if !mode_is_email_verify(&state.repo.db).await {
        return None;
    }
    let unverified: Option<Option<String>> = sqlx::query_scalar(
        "SELECT email FROM users WHERE id = $1 \
         AND email_verified_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .flatten();
    unverified.map(|_| {
        DomainError::Validation(format!("EMAIL_NOT_VERIFIED:{user_id}"))
    })
}

#[derive(Deserialize)]
struct VerifyQuery {
    token: String,
}

/// GET /auth/email/verify?token=...：一次性激活（幂等：重复点击同 token 提示已激活）。
#[get("/auth/email/verify")]
pub(crate) async fn email_verify(
    state: Db,
    q: web::Query<VerifyQuery>,
) -> DomainResult<HttpResponse> {
    let token = q.token.trim();
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DomainError::Validation("验证链接无效".into()));
    }
    let hash = sha3_256_hex(token);
    let row: Option<(i64, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT user_id, verified_at FROM email_verifications \
             WHERE token_hash = $1",
        )
        .bind(&hash)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((user_id, verified_at)) = row else {
        return Err(DomainError::Validation("验证链接无效或已被使用".into()));
    };
    if verified_at.is_some() {
        return Ok(ok(serde_json::json!({
            "verified": true, "already": true,
        })));
    }
    let updated = sqlx::query(
        "UPDATE email_verifications SET verified_at = now() \
         WHERE token_hash = $1 AND verified_at IS NULL \
         AND expires_at > now()",
    )
    .bind(&hash)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "验证链接已过期，请登录后重新发送".into(),
        ));
    }
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(user_id), "email_verified", Some(user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "verified": true, "already": false }),
    ))
}

#[derive(Deserialize)]
struct ResendBody {
    #[serde(default)]
    email: String,
}

/// POST /auth/email/resend：未激活用户补发验证信。
/// 鉴权二选一：带 token（被拦截后的会话拿不到 token，走 email+IP 限流）。
#[post("/auth/email/resend")]
pub(crate) async fn email_resend(
    state: Db,
    req: HttpRequest,
    body: web::Json<ResendBody>,
) -> DomainResult<HttpResponse> {
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    let ip = client_ip(&req);
    throttle(&state, format!("ev:resend:{email}:{ip}")).await?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, username FROM users \
         WHERE lower(email) = $1 AND email_verified_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, username)) = row else {
        // 不暴露「该邮箱是否存在」（枚举防护）——统一回成功文案
        return Ok(ok(serde_json::json!({ "sent": true })));
    };
    send_verification(&state, uid, &email, &username).await;
    Ok(ok(serde_json::json!({ "sent": true })))
}

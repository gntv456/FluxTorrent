//! 邀请邮件发送（M23）：把已有未用邀请码直接发到对方邮箱（NP invite.php「发送」口径）。
//! 从 invite_http.rs 机械外移。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub(super) struct EmailInviteReq {
    pub invite_id: i64,
    pub email: String,
}

/// SMTP 未配置时返回明确错误（开发态无邮件出口），前端提示改为复制邀请码。
#[post("/invites/email")]
pub(super) async fn email_invite_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EmailInviteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(DomainError::Validation("邮箱地址无效".into()));
    }
    // 邮箱黑名单（与注册同口径）：无命中行为 allow，命中且非 allow 行才禁
    let banned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM email_bans \
         WHERE mode <> 'allow' \
           AND (lower($1) = lower(pattern) \
                OR (pattern LIKE '@%' AND lower($1) LIKE '%' || lower(pattern)) \
                OR (pattern LIKE '%@' AND lower($1) LIKE lower(pattern) || '%')))",
    )
    .bind(&email)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if banned {
        return Err(DomainError::Validation("该邮箱地址已被站点禁用".into()));
    }
    // 归属 + 状态校验：只能发自己名下未用未过期的码
    let row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT code, expires_at FROM invites \
         WHERE id = $1 AND inviter_id = $2 AND status = 0 AND expires_at > now()",
    )
    .bind(body.invite_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((code, expires)) = row else {
        return Err(DomainError::Validation(
            "邀请码不存在或已使用/过期".into(),
        ));
    };

    // 0208 P0：SMTP「站点设定优先，env 兜底」——后台填好 smtp_* 即生效
    let smtp_cfg = crate::mailer::smtp_config(&state.repo.db).await;
    let Some((smtp, from)) = smtp_cfg.map(|c| (c.url, c.from)) else {
        // 开发态无邮件出口：仍登记发送对象，前端引导用户直接复制邀请码
        sqlx::query("UPDATE invites SET email = $2 WHERE id = $1")
            .bind(body.invite_id)
            .bind(&email)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Err(DomainError::Validation(
            "站点未配置邮件服务（SMTP），请复制邀请码手动发送给对方".into(),
        ));
    };
    let base = std::env::var("PUBLIC_WEB_URL")
        .or_else(|_| std::env::var("PUBLIC_API_URL"))
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let link = format!("{base}/register?invite={code}");
    let site: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "FluxTorrent".into());
    let inviter: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_else(|_| "某人".into());
    let hours_left = (expires - chrono::Utc::now()).num_hours().max(1);
    let mailer = crate::gaps_http::build_smtp(&smtp)?;
    let letter = lettre::Message::builder()
        .from(from.parse().map_err(|_| {
            DomainError::Validation("SMTP_FROM 配置无效".into())
        })?)
        .to(email
            .parse()
            .map_err(|_| DomainError::Validation("邮箱地址无效".into()))?)
        .subject(format!("[{site}] {inviter} 邀请你加入"))
        .body(format!(
            "你的好友 {inviter} 邀请你加入 {site}！

打开以下链接注册（邀请码 {hours_left} 小时内有效，仅可使用一次）：
{link}

邀请码：{code}

如不认识对方请忽略本邮件。"
        ))
        .map_err(|e| DomainError::Internal(e.into()))?;
    use lettre::AsyncTransport as _;
    mailer
        .send(letter)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE invites SET email = $2, emailed = true WHERE id = $1")
        .bind(body.invite_id)
        .bind(&email)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "invite_email", Some(body.invite_id))
        .await;
    Ok(ok(serde_json::json!({ "sent": true, "email": email })))
}

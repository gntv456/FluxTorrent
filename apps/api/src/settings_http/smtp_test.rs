//! POST /admin/settings/smtp-test：SMTP 连通性测试（五路方案 P0-2.3）。
//! 装配口径与投递链路完全同源（mailer::smtp_config + send_generic_mail），
//! 发一封测试信到指定收件人（缺省当前管理员邮箱）。找回密码/邀请函都走
//! 同一装配——这里通了，主链路就通。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
struct SmtpTestReq {
    /// 测试收件人；缺省发当前管理员自己的邮箱（无邮箱则报错提示）
    #[serde(default)]
    to: Option<String>,
}

#[post("/admin/settings/smtp-test")]
pub(super) async fn settings_smtp_test(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SmtpTestReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let to = match body.to.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t.to_string(),
        None => {
            let email: Option<String> = sqlx::query_scalar(
                "SELECT email FROM users WHERE id = $1",
            )
            .bind(auth.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
            email.ok_or_else(|| {
                DomainError::Validation(
                    "当前账号没有邮箱，请填写测试收件人地址".into(),
                )
            })?
        }
    };
    let cfg = crate::mailer::smtp_config(&state.repo.db)
        .await
        .ok_or_else(|| {
            DomainError::Validation(
                "SMTP 未配置：请先在「邮件」分组填写 SMTP 服务器与发件人".into(),
            )
        })?;
    let site: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'site_name'), 'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let r = crate::gaps_http::send_generic_mail(
        &cfg.url,
        &cfg.from,
        &to,
        &format!("[{site}] SMTP 测试邮件"),
        "这是一封测试邮件：如果你收到了它，说明站点 SMTP 配置生效，\
         找回密码/邀请函等通知可以正常投递。",
    )
    .await;
    match r {
        Ok(()) => Ok(ok(serde_json::json!({
            "sent": true, "to": to, "from": cfg.from,
        }))),
        Err(e) => Err(DomainError::Validation(format!(
            "测试邮件发送失败：{e:#}"
        ))),
    }
}

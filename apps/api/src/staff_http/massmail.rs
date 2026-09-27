//! 群发邮件。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/massmail")]
pub async fn massmail_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    let rows: Vec<MassMailRow> = sqlx::query_as(
        "SELECT m.id, m.subject, m.recipients, m.created_at, u.username AS sender \
         FROM mass_mails m LEFT JOIN users u ON u.id = m.sent_by ORDER BY m.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/massmail")]
pub async fn massmail_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MassMailBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let users: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO mass_mails (subject, body, sent_by, recipients) \
         VALUES ($1,$2,$3,$4) RETURNING id",
    )
    .bind(body.subject.trim())
    .bind(&body.body)
    .bind(auth.id)
    .bind(users as i32)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "massmail_send", None).await;
    // 审计修复（P1 永不投递）：注释宣称 worker 投递，但 worker 无任何 mass_mails 消费——
    // 邮件永不发出且响应报 queued 误导操作者。现在当场投递（复用 build_smtp）：
    // SMTP 配置时逐户发送并回填实发数；未配置时（开发态）明确返回 delivered=0 与
    // 原因，不再谎报入队。
    // 0208 P0：SMTP「站点设定优先，env 兜底」（与 mailer::smtp_config 同源）
    let cfg = crate::mailer::smtp_config(&state.repo.db).await;
    let mut delivered: i64 = 0;
    let mut mail_err: Option<String> = None;
    let note = match cfg {
        None => {
            mail_err = Some(
                "SMTP 未配置（站点设定与 SMTP_URL 均为空，邮件未投递，仅留档）"
                    .into(),
            );
            mail_err.unwrap()
        }
        Some(cfg) => {
            match crate::gaps_http::build_smtp(&cfg.url) {
                Ok(mailer) => {
                    let emails: Vec<String> = sqlx::query_scalar(
                        "SELECT email FROM users WHERE status \
                         < 2 AND email IS NOT NULL",
                    )
                    .fetch_all(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                    for to in &emails {
                        let msg = lettre::Message::builder()
                            .from(cfg.from.parse().map_err(
                                |e: lettre::address::AddressError| {
                                    DomainError::Internal(e.into())
                                },
                            )?)
                            .to(to.parse().map_err(
                                |e: lettre::address::AddressError| {
                                    DomainError::Internal(e.into())
                                },
                            )?)
                            .subject(body.subject.trim())
                            .body(body.body.clone())
                            .map_err(|e| DomainError::Internal(e.into()))?;
                        use lettre::AsyncTransport;
                        match mailer.send(msg).await {
                            Ok(_) => delivered += 1,
                            Err(e) => {
                                mail_err = Some(format!(
                                    "第 {delivered} 封后失败：{e}"
                                ));
                                break;
                            }
                        }
                    }
                }
                Err(e) => mail_err = Some(format!("SMTP 构建失败：{e}")),
            }
            mail_err.unwrap_or_else(|| "已全部投递".into())
        }
    };
    if delivered > 0 {
        let _ =
            sqlx::query("UPDATE mass_mails SET recipients = $2 WHERE id = $1")
                .bind(id)
                .bind(delivered as i32)
                .execute(&state.repo.db)
                .await;
    }
    Ok(ok(serde_json::json!({
        "id": id, "delivered": delivered,
        "queued": 0,
        "note": note,
    })))
}

#[derive(Deserialize)]
pub(super) struct MassMailBody {
    pub(super) subject: String,
    pub(super) body: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct MassMailRow {
    pub(super) id: i32,
    pub(super) subject: String,
    pub(super) recipients: i32,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    pub(super) sender: Option<String>,
}

//! 申请制入站（0227，P2 触点 #3，UNIT3D application 口径）：
//! 站长开启 `application_signup=open` 后，匿名用户在 /apply 提交举证
//! （佐证链接 + 理由），staff 后台人审；通过自动生成邀请码（72h，
//! 绑定申请邮箱），拒绝可附理由。防滥用：IP 限流 + 同邮箱待审去重。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, require_auth, throttle};
use crate::state::AppState;

async fn application_open(db: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'application_signup'), 'off') = 'open'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

#[derive(Deserialize)]
pub(super) struct ApplyReq {
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub proof_links: String,
    #[serde(default)]
    pub reason: String,
}

/// 提交申请（匿名；通道开着才有 200，否则 404 语义的 Validation）
#[post("/apply")]
pub(super) async fn apply_submit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ApplyReq>,
) -> DomainResult<HttpResponse> {
    let ip = client_ip(&req);
    throttle(&state, format!("apply-ip:{ip}")).await?;
    if !application_open(&state.repo.db).await {
        return Err(DomainError::Validation("本站未开放申请通道".into()));
    }
    let username = body.username.trim().to_string();
    let email = body.email.trim().to_lowercase();
    if username.len() < 2
        || username.len() > 24
        || !email.contains('@')
        || body.reason.trim().len() < 20
    {
        return Err(DomainError::Validation(
            "用户名 2-24 字符、邮箱有效、申请理由至少 20 字".into(),
        ));
    }
    // 同邮箱待审去重 + 用户名/邮箱已被占用拒绝
    let dup: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM applications \
         WHERE email = $1 AND status = 0",
    )
    .bind(&email)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if dup.is_some() {
        return Err(DomainError::Validation(
            "该邮箱已有待审申请，请耐心等待".into(),
        ));
    }
    let taken: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users \
         WHERE username = $1 OR lower(email) = $2",
    )
    .bind(&username)
    .bind(&email)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if taken.is_some() {
        return Err(DomainError::Validation("用户名或邮箱已被注册".into()));
    }
    // 佐证链接至少一条（举证门槛；无他站佐证的申请拒绝率高，没意义）
    if body.proof_links.trim().is_empty() {
        return Err(DomainError::Validation(
            "请至少提供一条佐证链接（其他站点个人主页等）".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO applications \
         (username, email, proof_links, reason) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&username)
    .bind(&email)
    .bind(body.proof_links.trim())
    .bind(body.reason.trim())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "submitted": true,
        "note": "申请已提交，审核结果将通过邮件通知（请确保邮箱有效）",
    })))
}

/// 后台队列（staff）：status 筛选 + 决断
#[get("/admin/applications")]
pub(super) async fn apply_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    let rows: Vec<(i64, String, String, String, String, i16)> = sqlx::query_as(
        "SELECT id, username, email, proof_links, reason, status \
             FROM applications ORDER BY (status = 0) DESC, id DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "items": rows.iter().map(|(id, u, e, l, r, s)| serde_json::json!({
            "id": id, "username": u, "email": e,
            "proof_links": l, "reason": r, "status": s,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub(super) struct DecideReq {
    pub id: i64,
    /// approve | reject
    pub action: String,
    #[serde(default)]
    pub note: String,
}

/// 决断（staff）：通过 → 生成绑定邮箱的邀请码并记录；拒绝 → 落备注。
/// 邮件通知：SMTP 可用时发结果（失败只落日志不阻断）。
#[post("/admin/applications/decide")]
pub(super) async fn apply_decide(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DecideReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    let approve = match body.action.as_str() {
        "approve" => true,
        "reject" => false,
        _ => {
            return Err(DomainError::Validation(
                "action 仅支持 approve / reject".into(),
            ))
        }
    };
    let row: Option<(String, String, i16)> = sqlx::query_as(
        "SELECT username, email, status FROM applications \
         WHERE id = $1 FOR UPDATE",
    )
    .bind(body.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((username, email, status)) = row else {
        return Err(DomainError::NotFound(body.id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该申请已被处理".into()));
    }
    let mut invite_code: Option<String> = None;
    if approve {
        let code = crate::domain::new_invite_code();
        let expires = chrono::Utc::now() + chrono::Duration::hours(72);
        sqlx::query(
            "INSERT INTO invites (inviter_id, code, expires_at, email) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(auth.id)
        .bind(&code)
        .bind(expires)
        .bind(&email)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        invite_code = Some(code.clone());
        sqlx::query(
            "UPDATE applications \
             SET status = 1, handled_by = $2, handled_at = now(), \
                 decide_note = $3, invite_code = $4 WHERE id = $1",
        )
        .bind(body.id)
        .bind(auth.id)
        .bind(body.note.trim())
        .bind(&code)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query(
            "UPDATE applications \
             SET status = 2, handled_by = $2, handled_at = now(), \
                 decide_note = $3 WHERE id = $1",
        )
        .bind(body.id)
        .bind(auth.id)
        .bind(body.note.trim())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 结果邮件（尽力而为）
    let cfg = crate::mailer::smtp_config(&state.repo.db).await;
    if let Some(cfg) = cfg {
        let site: String = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \
                WHERE name = 'site_name'), 'FluxTorrent')",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "FluxTorrent".into());
        let (subject, body_text) = if approve {
            (
                format!("[{site}] 你的入站申请已通过"),
                format!(
                    "你好 {username}，你的入站申请已通过。\
                     \n邀请码：{}\n（72 小时内有效，用邮箱 {} 注册）",
                    invite_code.clone().unwrap_or_default(),
                    email
                ),
            )
        } else {
            (
                format!("[{site}] 你的入站申请未通过"),
                format!(
                    "你好 {username}，你的入站申请未通过。{}\n欢迎完善材料后再次申请。",
                    if body.note.trim().is_empty() {
                        String::new()
                    } else {
                        format!("理由：{}", body.note.trim())
                    }
                ),
            )
        };
        let to = email.clone();
        actix_web::rt::spawn(async move {
            let _ = crate::gaps_http::send_generic_mail(
                &cfg.url, &cfg.from, &to, &subject, &body_text,
            )
            .await;
        });
    }
    state
        .repo
        .audit(Some(auth.id), "application_decide", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({
        "decided": true,
        "invite_code": invite_code,
    })))
}

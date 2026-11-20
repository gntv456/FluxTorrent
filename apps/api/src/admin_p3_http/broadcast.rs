//! 运营广播双口（0505 运营体检批）：
//! ① POST /admin/pm-broadcast —— 站内信批量广播。此前站内信没有独立
//!    群发口：massmail 只入邮件通道（mail_outbox），全员/圈选站内信
//!    只能搭「批量发放」的附属通知（increment_bulk→bulk_notify）。
//!    本口复用 massmail 的 0497 圈选口径（all/active30/dormant90/vip/
//!    class:N 白名单，无自由文本进 SQL）+ bulk_notify 的 system 档
//!    偏好闸（用户关掉「系统通知」不收，必达类仍走 mailer::notify）。
//! ② POST /admin/shout-broadcast —— 聊天室官方广播。此前系统播报
//!    只藏在促销 job（promos.rs），后台无法手动发一条官方喊话。
//!    广播行 sender 侧 user_id=NULL（0505 迁移放宽可空，系统身份行），
//!    读侧 username 本就 LEFT JOIN 可空、前端按 username 缺省渲染系统名。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 0497 同款圈选（与 massmail::audience_pred 字节级同口径；两边各自
/// 独立小函数，跨模块抽公共反而把 staff_http 与 admin_p3_http 焊死）。
fn audience_pred(audience: Option<&str>) -> String {
    match audience.unwrap_or("all") {
        "active30" => " AND COALESCE(u.last_seen_at, u.created_at) \
             > now() - interval '30 days'"
            .into(),
        "dormant90" => " AND COALESCE(u.last_seen_at, u.created_at) \
             <= now() - interval '90 days'"
            .into(),
        "vip" => " AND (u.donor OR u.vip_until > now() \
                 OR u.donor_until > now())"
            .into(),
        _ => String::new(),
    }
}

/// audience 参数解析（class:N 特殊档，与 massmail_send 同规）。
fn aud_parse(aud: Option<&str>) -> DomainResult<(String, String)> {
    let aud = aud.unwrap_or("all");
    if let Some(n) = aud.strip_prefix("class:") {
        let n: i32 = n.parse().map_err(|_| {
            DomainError::Validation("audience class:N 的 N 须为数字".into())
        })?;
        if !(1..=12).contains(&n) {
            return Err(DomainError::Validation(
                "audience class:N 的 N 须在 1-12".into(),
            ));
        }
        return Ok((format!(" AND u.class_id >= {n}"), aud.to_string()));
    }
    Ok((audience_pred(Some(aud)), aud.to_string()))
}

#[derive(Deserialize)]
pub struct PmBroadcastReq {
    pub subject: String,
    pub body: String,
    /// all | active30 | dormant90 | vip | class:N（1-12）
    #[serde(default)]
    pub audience: Option<String>,
}

/// POST /admin/pm-broadcast —— 站内信批量广播（MASSMAIL 权限）。
#[post("/admin/pm-broadcast")]
pub async fn pm_broadcast(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PmBroadcastReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let (pred, aud_note) = aud_parse(body.audience.as_deref())?;
    // 圈选计数（与 massmail 预统计同口径；status<2 排除封禁/注销）
    let total: i64 = {
        let sql =
            format!("SELECT count(*) FROM users u WHERE u.status < 2{pred}");
        sqlx::query_scalar(&sql)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
    };
    if total == 0 {
        return Err(DomainError::Validation("圈选人群为空，未发送".into()));
    }
    // 分块入队（500/批，同 bulk_notify chunk 口径）；INSERT ... SELECT
    // 一次插一批，偏好闸 u_notice_enabled 在 SELECT 里逐行判。
    let sender = auth.id;
    let subject = body.subject.trim().to_string();
    let text = body.body.trim().to_string();
    let mut sent: i64 = 0;
    for lo in (0..total).step_by(500) {
        let sql = format!(
            "INSERT INTO messages \
             (sender_id, receiver_id, subject, body, kind) \
             SELECT $1, u.id, $3, $4, 'system' \
             FROM (SELECT u.id FROM users u WHERE u.status < 2{pred} \
                   ORDER BY u.id LIMIT 500 OFFSET {lo}) u \
             WHERE u_notice_enabled(u.id, 'system')"
        );
        let r = sqlx::query(&sql)
            .bind(sender)
            .bind(total as i64)
            .bind(&subject)
            .bind(&text)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        sent += r.rows_affected() as i64;
    }
    state
        .repo
        .audit(Some(auth.id), "ops.pm_broadcast", Some(sent))
        .await;
    Ok(ok(serde_json::json!({
        "targeted": total,
        "sent": sent,
        "skipped_by_pref": total - sent,
        "audience": aud_note,
    })))
}

#[derive(Deserialize)]
pub struct ShoutBroadcastReq {
    pub message: String,
}

/// POST /admin/shout-broadcast —— 聊天室官方广播（ANNOUNCE_PUBLISH）。
/// sender_id=NULL＝系统身份行；300 字同普通喊话上限。
#[post("/admin/shout-broadcast")]
pub async fn shout_broadcast(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutBroadcastReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::ANNOUNCE_PUBLISH,
    )
    .await?;
    let trimmed = body.message.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 300 {
        return Err(DomainError::Validation("广播需 1-300 字".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO shoutbox (user_id, message) \
         VALUES (NULL, $1) RETURNING id",
    )
    .bind(trimmed)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "ops.shout_broadcast", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

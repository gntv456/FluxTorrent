//! 邀请（M23 P0 面）：配额/发放/邮件/兑换。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

use crate::dto::ok;
use crate::economy_http::{spend_spark, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, AuthUser};
use crate::state::AppState;

// ============ 邀请（M23 P0 面） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct InviteRow {
    id: i64,
    code: String,
    status: i16,
    used_by: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
    /// NP invite.php 口径：发送对象邮箱（邮件邀请才有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    email: Option<String>,
    /// 邮件是否已投递（发送失败为 false，用户可重发）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    emailed: Option<bool>,
}

/// 判定用户邀请配额上限（与 issue 消耗路径同一口径）：
/// INVITES_BONUS 权限（外联员/VIP）4 枚/周；LV3+ 2 枚/周；其余 0（只能魔力兑换）。
pub async fn invite_quota_limit(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
) -> i64 {
    if crate::authz::can(state, auth, crate::authz::perm::INVITES_BONUS).await {
        4
    } else if auth.class_id >= 3 {
        2
    } else {
        0
    }
}

/// 我的邀请概览（NP invite.php 顶部口径）：配额用量 + 魔力兑换现价 + 剩余额外配额。
/// 前端据此禁用不可用按钮，而不是点了才吃 403。
#[get("/invites/status")]
pub async fn invites_status_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let limit = invite_quota_limit(&state, &auth).await;
    let (used, extra): (i32, i32) = sqlx::query_as(
        "SELECT \
            COALESCE((SELECT used FROM invite_quota \
                      WHERE user_id = $1 AND period = date_trunc('week', now())::date), 0), \
            COALESCE(quota_extra, 0) \
         FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or((0, 0));
    let price: Option<i64> = sqlx::query_scalar(
        "SELECT price FROM shop_items WHERE kind = 'invite' AND active = true ORDER BY price LIMIT 1",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let inv_stats: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 0 AND expires_at > now()), \
                count(*) FILTER (WHERE status = 1), \
                count(*) FILTER (WHERE status = 0 AND expires_at <= now()) \
         FROM invites WHERE inviter_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (unused, used_count, expired) = inv_stats;
    Ok(ok(serde_json::json!({
        "class_id": auth.class_id,
        "quota_limit": limit,
        "quota_used": used,
        "quota_extra": extra,
        "redeem_price": price,
        "unused": unused, "used": used_count, "expired": expired,
    })))
}

#[get("/invites")]
pub async fn list_invites_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // status=2（已过期）在下发时即时折算：过期未用的码视为失效（注册侧已拒绝 expires_at > now()）
    let rows = sqlx::query_as::<_, InviteRow>(
        "SELECT i.id, i.code, \
                (CASE WHEN i.status = 0 AND i.expires_at <= now() THEN 2 ELSE i.status END)::int2 AS status, \
                u.username AS used_by, i.expires_at, i.email, i.emailed \
         FROM invites i LEFT JOIN users u ON u.id = i.used_by \
         WHERE i.inviter_id = $1 ORDER BY i.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/invites")]
pub async fn issue_invite_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 配额：等级 LV3+ 每周 2 枚；持有 invites.bonus（外联员 / VIP）提升为 4 枚。
    // quota_extra（捐赠/管理发放的额外配额）优先于周配额消耗（0066 修复：此前只加不扣，
    // 用户永远领不到这部分额外邀请）。
    let extra: Option<i32> = sqlx::query_scalar(
        "UPDATE users SET quota_extra = quota_extra - 1 WHERE id = $1 AND quota_extra > 0 RETURNING quota_extra",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if extra.is_none() {
        // 额外配额不够 → 走周配额原子占位（UPDATE 计数行防并发穿透）
        let quota = invite_quota_limit(&state, &auth).await;
        if quota == 0 {
            return Err(DomainError::Validation(
                "等级达到 LV3 后才能生成邀请码；也可用魔力兑换（不受等级限制）"
                    .into(),
            ));
        }
        sqlx::query(
            "INSERT INTO invite_quota (user_id, period, used) VALUES ($1, date_trunc('week', now())::date, 0) ON CONFLICT DO NOTHING",
        )
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let taken: Option<i32> = sqlx::query_scalar(
            "UPDATE invite_quota SET used = used + 1 WHERE user_id = $1 AND period = date_trunc('week', now())::date AND used < $2 RETURNING used",
        )
        .bind(auth.id)
        .bind(quota)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if taken.is_none() {
            return Err(DomainError::Validation(
                "本周邀请配额已用完，下周重置；也可用魔力兑换".into(),
            ));
        }
    }
    let code = crate::domain::new_invite_code();
    let expires = crate::domain::invite_expiry();
    let id = state.repo.issue_invite(auth.id, &code, expires).await?;
    Ok(ok(
        serde_json::json!({ "id": id, "code": code, "expires_at": expires.to_rfc3339() }),
    ))
}

#[derive(Deserialize)]
struct EmailInviteReq {
    invite_id: i64,
    email: String,
}

/// 把已有未用邀请码直接发到对方邮箱（NP invite.php「发送」口径）。
/// SMTP 未配置时返回明确错误（开发态无邮件出口），前端提示改为复制邀请码。
#[post("/invites/email")]
pub async fn email_invite_handler(
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

    let smtp = std::env::var("SMTP_URL").unwrap_or_default();
    if smtp.is_empty() {
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
    }
    let base = std::env::var("PUBLIC_WEB_URL")
        .or_else(|_| std::env::var("PUBLIC_API_URL"))
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let link = format!("{base}/register?invite={code}");
    let from = std::env::var("SMTP_FROM")
        .unwrap_or_else(|_| "no-reply@fluxtorrent.local".into());
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

#[derive(Deserialize)]
struct RedeemInviteReq {
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 魔力兑换邀请：按魔力商店「邀请名额」（kind=invite）现价扣火花，直接发一枚 72h 邀请码。
/// 无等级限制（商店购买口径），与 /shop/buy 走同一条扣款管线（幂等键防双扣）。
#[post("/invites/redeem")]
pub async fn redeem_invite_handler(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<RedeemInviteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let item: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, price FROM shop_items WHERE kind = 'invite' AND active = true ORDER BY price LIMIT 1",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((item_id, price)) = item else {
        return Err(DomainError::Validation("商店未开放邀请兑换".into()));
    };
    // 幂等键必填（与 /shop/buy 同口径）：网络重试携带同一键防双扣款；
    // 服务端键带 uid 前缀，防裸客户端键跨用户碰撞误判重放
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.is_empty())
        .map(|k| format!("invite_redeem:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "shop_item",
        item_id,
    )
    .await?;
    if matches!(outcome, SpendOutcome::Replayed) {
        // 幂等重放：码已在首次请求发放，不重复发；前端刷新列表即可看到
        return Ok(ok(serde_json::json!({ "replayed": true })));
    }
    let code = crate::domain::new_invite_code();
    let expires = crate::domain::invite_expiry();
    let id = state.repo.issue_invite(auth.id, &code, expires).await?;
    state
        .repo
        .audit(Some(auth.id), "invite_redeem", Some(id))
        .await;
    Ok(ok(serde_json::json!({
        "id": id, "code": code, "expires_at": expires.to_rfc3339(), "price": price,
    })))
}

//! 邀请（M23 P0 面）：配额/发放/邮件/兑换。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」）。
//! 0204：配置化（周配额/TTL 迁 site_settings）+ 撤销 + 被邀请人列表 + 分页。

mod email;

pub use email::email_invite_handler;

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
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

/// 设置键点查（i64）：读不到/解析失败回退缺省。
async fn setting_i64(db: &sqlx::PgPool, name: &str, default: i64) -> i64 {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .and_then(|v: String| v.parse().ok())
        .unwrap_or(default)
}

/// 用户邀请码 TTL（0204 配置化）：site_settings.invite_ttl_hours，缺省 72h。
async fn invite_ttl_hours(db: &sqlx::PgPool) -> i64 {
    (setting_i64(db, "invite_ttl_hours", 72).await).clamp(1, 720)
}

/// 判定用户邀请配额上限（与 issue 消耗路径同一口径）：
/// INVITES_BONUS 权限（外联员/VIP）与 LV3+ 的每周配额——
/// 0204 起读 site_settings（invites_weekly_bonus / invites_weekly_class3）。
pub async fn invite_quota_limit(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
) -> i64 {
    if crate::authz::can(state, auth, crate::authz::perm::INVITES_BONUS).await {
        (setting_i64(&state.repo.db, "invites_weekly_bonus", 4).await)
            .clamp(0, 100)
    } else if auth.class_id >= 3 {
        (setting_i64(&state.repo.db, "invites_weekly_class3", 2).await)
            .clamp(0, 100)
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
        "SELECT price FROM shop_items WHERE kind = 'invite' AND active \
         = true ORDER BY price LIMIT 1",
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

#[derive(Deserialize)]
struct ListQuery {
    page: Option<i64>,
}

#[get("/invites")]
pub async fn list_invites_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 0204 分页：此前 LIMIT 50 截断重度用户历史；页大小 50。
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per = 50i64;
    // status=2（已过期）在下发时即时折算：过期未用的码视为失效（注册侧已拒绝 expires_at > now()）
    let rows = sqlx::query_as::<_, InviteRow>(
        "SELECT i.id, i.code, \
                (CASE WHEN i.status = 0 AND i.expires_at <= now() THEN 2 ELSE i.status END)::int2 AS status, \
                u.username AS used_by, i.expires_at, i.email, i.emailed \
         FROM invites i LEFT JOIN users u ON u.id = i.used_by \
         WHERE i.inviter_id = $1 ORDER BY i.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.id)
    .bind(per)
    .bind((page - 1) * per)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invites WHERE inviter_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "page": page,
        "per": per,
    })))
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
        "UPDATE users SET quota_extra = quota_extra - 1 WHERE id = $1 \
         AND quota_extra > 0 RETURNING quota_extra",
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
            "INSERT INTO invite_quota (user_id, period, used) \
             VALUES ($1, date_trunc('week', now())::date, 0) ON CONFLICT DO \
             NOTHING",
        )
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let taken: Option<i32> = sqlx::query_scalar(
            "UPDATE invite_quota SET used = used + 1 WHERE user_id \
             = $1 AND period = date_trunc('week', now())::date AND used < $2 \
             RETURNING used",
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
    let expires = chrono::Utc::now()
        + chrono::Duration::hours(invite_ttl_hours(&state.repo.db).await);
    let id = state.repo.issue_invite(auth.id, &code, expires).await?;
    Ok(ok(
        serde_json::json!({ "id": id, "code": code, "expires_at": expires.to_rfc3339() }),
    ))
}

/// 撤销自己的未用邀请码（0204 P0）：status 0→3。
/// 不返还周配额/quota_extra（防「生成→撤销」刷配额）；邮件已发的码撤销
/// 后注册侧自然拒绝（status=0 CAS 不匹配）。
#[delete("/invites/{id}")]
pub async fn revoke_invite_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE invites SET status = 3 \
         WHERE id = $1 AND inviter_id = $2 AND status = 0",
    )
    .bind(id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "邀请码不存在、已使用或已撤销".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "invite.revoke", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "revoked": id })))
}

/// 被邀请人列表（0204 P0，NP invitee 口径）：上传/下载/分享率/状态。
/// 邮箱脱敏口径同个人主页（不回传 email）。
#[get("/invites/invitees")]
pub async fn list_invitees_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<serde_json::Value> =
        sqlx::query_as::<_, (i64, String, i16, Option<String>, i64, i64, chrono::DateTime<chrono::Utc>)>(
        "SELECT u.id, u.username, u.status, c.name AS class_name, u.uploaded, u.downloaded, u.created_at \
         FROM users u LEFT JOIN user_classes c ON c.id = u.class_id \
         WHERE u.invited_by = $1 ORDER BY u.id DESC LIMIT 200",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, username, status, class_name, up, down, created_at)| {
        serde_json::json!({
            "id": id, "username": username, "status": status,
            "class_name": class_name,
            "uploaded": up, "downloaded": down,
            "created_at": created_at.to_rfc3339(),
        })
    })
    .collect();
    Ok(ok(rows))
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
                "SELECT id, \
         price FROM shop_items WHERE kind = 'invite' AND active = true ORDER BY price LIMIT 1",
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
    let expires = chrono::Utc::now()
        + chrono::Duration::hours(invite_ttl_hours(&state.repo.db).await);
    let id = state.repo.issue_invite(auth.id, &code, expires).await?;
    state
        .repo
        .audit(Some(auth.id), "invite_redeem", Some(id))
        .await;
    Ok(ok(serde_json::json!({
        "id": id, "code": code, "expires_at": expires.to_rfc3339(), "price": price,
    })))
}

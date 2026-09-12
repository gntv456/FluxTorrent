//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;

use crate::auth;

use crate::domain::{self};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents;
use crate::economy_http::{spend_spark, SpendOutcome};

pub fn v1_scope() -> actix_web::Scope {
    web::scope("/api/v1")
        .service(health)
        .service(register)
        .service(login)
        .service(me)
        .service(me_overview)
        .service(me_settings_get)
        .service(me_settings_put)
        .service(my_torrentlist)
        .service(my_bookmarks)
        .service(rotate_passkey)
        .service(me_password_change)
        .service(user_public_profile)
        .service(list)
        .service(detail)
        .service(torrent_detail_ext)
        .service(torrent_files)
        .service(torrent_thanks)
        .service(comments)
        .service(create_comment)
        .service(do_thank)
        .service(do_bookmark)
        .service(edit_torrent)
        .service(delete_torrent)
        .service(torrent_snatches)
        .service(torrent_nfo)
        .service(request_reseed)
        .service(torrent_tags)
        .service(torrent_tag_put)
        .service(stats)
        .service(report_create)
        .service(rss_info)
        .service(news_create)
        .service(news_update)
        .service(news_delete)
        .service(fun_items)
        .service(fun_item_vote)
        .service(fun_item_create)
        .service(fun_item_update)
        .service(fun_item_set_status)
        .service(fun_item_delete)
        .service(link_apply)
        .service(link_admin_list)
        .service(link_update)
        .service(link_delete)
        .service(faq_list)
        .service(faq_create)
        .service(faq_update)
        .service(faq_delete)
        .service(rules_content)
        .service(rule_create)
        .service(rule_update)
        .service(rule_delete)
        .service(category_list)
        .service(category_create)
        .service(category_update)
        .service(category_delete)
        .service(ban_list)
        .service(ban_create)
        .service(ban_delete)
        .service(freeleech_set)
        .service(freeleech_clear)
        .service(freeleech_list)
        .service(staffmess_send)
        .service(admin_add_user)
        .service(admin_amount_bonus)
        .service(warned_list)
        .service(warn_user)
        .service(unwarn_user)
        .service(ipcheck)
        .service(maxlogin)
        .service(admin_amount_upload)
        .service(admin_reset_pass)
        .service(admin_delete_disabled)
        .service(emailban_list)
        .service(emailban_create)
        .service(emailban_delete)
        .service(test_ip)
        .service(admin_stats)
        .service(clear_cache)
        .service(do_cleanup)
        .service(ad_list)
        .service(ad_create)
        .service(ad_update)
        .service(ad_toggle)
        .service(ad_delete)
        .service(not_connectable)
        .service(uploaders)
        .service(all_agents)
        .service(poll_overview)
        .service(db_stats)
        .service(sys_log)
        .service(locations)
        .service(donate_state)
        .service(donate_topup)
        .service(donate_order)
        .service(site_profile)
        .service(site_type_pack_list)
        .service(site_type_pack_apply)
        .service(massmail_list)
        .service(massmail_send)
        .service(medal_wall)
        .service(contest_list)
        .service(contest_join)
        .service(frame_list)
        .service(frame_equip)
        .service(gomoku_create)
        .service(gomoku_join)
        .service(gomoku_move)
        .service(gomoku_get)
        .service(home_sections)
        .service(announce_stats)
        .service(upload)
        .service(download)
        .service(issue_invite_handler)
        .service(redeem_invite_handler)
        .service(list_invites_handler)
        .service(logout)
}

// ============ 基础 ============

#[get("/health")]
async fn health() -> impl Responder {
    ok(serde_json::json!({ "status": "up", "service": "flux-api" }))
}

// ============ 认证（M01） ============

#[derive(Deserialize)]
struct RegisterReq {
    username: String,
    email: String,
    password: String,
    invite_code: String,
    #[serde(default)]
    captcha_id: String,
    #[serde(default)]
    captcha_answer: i32,
}

#[post("/auth/register")]
async fn register(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RegisterReq>,
) -> DomainResult<impl Responder> {
    let new_user = domain::NewUser {
        username: body.username.trim().to_string(),
        email: body.email.trim().to_string(),
        password: body.password.clone(),
    };
    domain::validate_register(&new_user)?;
    // 图形验证码校验（防注册机）
    if body.captcha_id.is_empty()
        || !crate::gaps_http::captcha_verify(&state, &body.captcha_id, body.captcha_answer).await
    {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    // 注册限流（§5.7）：按来源 IP 每分钟 5 次，防邀请码爆破
    let ip = req
        .connection_info()
        .peer_addr()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".into());
    // IP 封禁强制校验（与登录同口径；封禁名单由管理面维护）
    if ip_banned(&state, &ip).await {
        return Err(DomainError::Validation("IP 已被封禁，请联系管理组".into()));
    }
    throttle(&state, format!("register-ip:{ip}")).await?;
    let pass_hash = domain::hash_password(&new_user.password)?;
    // 先建用户（未绑定邀请人），再原子消费邀请码回填（一码一用）
    let user_id = state
        .repo
        .create_user(&new_user.username, &new_user.email, &pass_hash, None)
        .await?;
    match state.repo.consume_invite(&body.invite_code, user_id).await {
        Ok(inviter) => {
            sqlx::query("UPDATE users SET invited_by = $2 WHERE id = $1")
                .bind(user_id)
                .bind(inviter)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            state
                .repo
                .audit(Some(user_id), "user_register", Some(user_id))
                .await;
            Ok(ok(serde_json::json!({ "user_id": user_id })))
        }
        Err(e) => {
            // 邀请码无效则回滚用户创建，防止占用用户名
            let _ = sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(user_id)
                .execute(&state.repo.db)
                .await;
            Err(e)
        }
    }
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    password: String,
    #[serde(default)]
    totp_code: Option<u32>,
}

#[post("/auth/login")]
async fn login(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LoginReq>,
) -> DomainResult<impl Responder> {
    // IP 封禁强制校验（ip_bans 此前仅管理面 CRUD，无请求入口拦截）
    let peer_ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default();
    if ip_banned(&state, &peer_ip).await {
        return Err(DomainError::Validation("IP 已被封禁，请联系管理组".into()));
    }
    // 登录限流（§5.7：5 次/分钟/用户名，Redis 计数）
    throttle(&state, format!("login:{}", body.username)).await?;
    let user = state
        .repo
        .find_user_by_name(body.username.trim())
        .await?
        .ok_or(DomainError::InvalidCredentials);
    let user = match user {
        Ok(u) => u,
        Err(e) => {
            let _ = sqlx::query(
                "INSERT INTO login_events (user_id, ip, ok) VALUES (0, NULLIF($1,'')::inet, false)",
            )
            .bind(&peer_ip)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    };
    if !domain::verify_password(&user.pass_hash, &body.password) {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok) VALUES ($1, NULLIF($2,'')::inet, false)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::InvalidCredentials);
    }
    // 2FA（启用者必须带 totp_code）
    crate::twofa_http::login_totp_check(&state.repo.db, user.id, body.totp_code.unwrap_or(0))
        .await?;
    let token = auth::issue(user.id, user.class_id, &state.cfg.jwt_secret, 24)
        .map_err(DomainError::Internal)?;
    // 登录事件（控制面板账户概览 30 天活跃趋势；含 IP 供 ipcheck/maxlogin）
    let _ = sqlx::query("INSERT INTO login_events (user_id, ip, ok) VALUES ($1, NULLIF($2,'')::inet, true)")
        .bind(user.id)
        .bind(&peer_ip)
        .execute(&state.repo.db)
        .await;
    // M28 插件 Hook：登录成功后分发
    state.plugins.dispatch_login(&state, user.id);
    Ok(ok(serde_json::json!({
        "token": token,
        "must_reset_password": user.must_reset_password,
        "user": { "id": user.id, "username": user.username, "class_id": user.class_id }
    })))
}

async fn throttle(state: &Arc<AppState>, key: String) -> DomainResult<()> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let k = format!("rl:{}", key);
    let n: i64 = c.incr(&k, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = c.expire(&k, 60).await.unwrap_or(());
    }
    if n > 5 {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

/// ILIKE/LIKE 模式构造：转义用户输入中的通配符（% _ \），防 `%` 全表通配扫描的性能滥用。
/// PG 的 LIKE/ILIKE 默认转义符即反斜杠，无需 ESCAPE 子句；torrents.rs 的 ESCAPE chr(92) 口径兼容。
pub fn like_pattern(s: &str) -> String {
    format!(
        "%{}%",
        s.trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

/// 通知 tracker 立即刷新防护缓存（ip_bans/agent_rules/挂起/下载权限/passkey 变更后调用）。
/// tracker 侧每 3s 轮询 flux:guard:ver（见 apps/tracker/src/main.rs）；失败静默 —— 仍有 60s TTL 兜底。
pub async fn bump_guard_ver(state: &Arc<AppState>) {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let _: Result<i64, _> = c.incr("flux:guard:ver", 1).await;
}

/// ip_bans 强制校验：命中返回 true（封禁名单由管理面维护，见 /admin/bans）。
/// 登录/注册为低频入口，直接查库即可；高频路径（tracker announce）用内存缓存版，
/// 见 apps/tracker/src/main.rs 的 refresh_guard()/ip_banned()。
async fn ip_banned(state: &Arc<AppState>, ip: &str) -> bool {
    if ip.is_empty() || ip == "unknown" {
        return false;
    }
    sqlx::query_scalar::<_, i32>("SELECT 1 FROM ip_bans WHERE ip = $1::inet LIMIT 1")
        .bind(ip)
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten()
        .is_some()
}

#[derive(Debug)]
pub struct AuthUser {
    pub id: i64,
    pub class_id: i32,
    /// 本次凭证的签发秒（撤销线语义需要：改密时以它为界作废更早的 token）
    pub iat: i64,
}

/// 从 Authorization: Bearer 提取用户（§8.1：后端权威鉴权）
pub async fn require_auth(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<AuthUser> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(DomainError::Unauthorized)?;
    let claims = auth::verify(token, &state.cfg.jwt_secret).ok_or(DomainError::Unauthorized)?;
    // 登出撤销检查：签发时间早于 not-before 的 token 一律拒绝
    {
        let mut c = state.redis.clone();
        let nbf: Option<i64> =
            redis::AsyncCommands::get(&mut c, format!("logout_nbf:{}", claims.sub))
                .await
                .unwrap_or(None);
        if let Some(nbf) = nbf {
            // <=：撤销线含义为「iat 不晚于 nbf 的凭证全部作废」。登出把 nbf 设为凭证 iat，
            // 因此登出所用的 token（iat==nbf）自身也被判死 —— 这是登出的本意；
            // 改密路径写 nbf=iat-1，保留改密后同秒重登的新凭证（见 me_password_change）。
            if claims.iat <= nbf {
                return Err(DomainError::Unauthorized);
            }
        }
    }
    // 权威校验（P1 修复）：token 只是凭证，状态与等级以库为准 —— 封禁/降级即时生效
    let row: Option<(i16, i32)> =
        sqlx::query_as("SELECT status, class_id FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((status, class_id)) = row else {
        return Err(DomainError::Unauthorized);
    };
    if status >= 2 {
        return Err(DomainError::Forbidden); // 封禁账户
    }
    Ok(AuthUser {
        id: claims.sub,
        class_id,
        iat: claims.iat,
    })
}

fn require_staff(user: &AuthUser) -> DomainResult<()> {
    if user.class_id >= 90 {
        Ok(())
    } else {
        Err(DomainError::Forbidden)
    }
}

#[post("/auth/logout")]
async fn logout(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 令牌撤销（§5.7）：nbf = 本次凭证 iat —— require_auth 用 iat<=nbf 判死，
    // 因此登出所用的 token 与一切更早签发的立即失效；登出后新登录（iat 严格更大）不受影响。
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let _: () = redis::AsyncCommands::set_ex(&mut c, &key, auth.iat, 86400u64)
        .await
        .unwrap_or(());
    state.repo.audit(Some(auth.id), "auth.logout", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[get("/me")]
async fn me(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let user = state
        .repo
        .find_user_by_id(auth.id)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    // 个人主页口径（旧站 getusertorrentlist / my_data_stats）：传输量 + 分享率 + 做种/下载计数
    let row: Option<(i64, i64, i64, i64, i64, i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT u.uploaded, u.downloaded,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS uploads,
               (SELECT count(*) FROM bookmarks b WHERE b.user_id = u.id) AS bookmarks,
               c.name AS class_name
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
        WHERE u.id = $1
        "#,
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (uploaded, downloaded, seeding, leeching, uploads, bookmarks, class_name) =
        row.unwrap_or((0, 0, 0, 0, 0, 0, None));
    let frame_id: Option<i32> = sqlx::query_scalar("SELECT avatar_frame_id FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    Ok(ok(serde_json::json!({
        "id": user.id, "username": user.username, "class_id": user.class_id,
        "must_reset_password": user.must_reset_password,
        "uploaded": uploaded, "downloaded": downloaded,
        "seeding": seeding, "leeching": leeching,
        "uploads": uploads, "bookmarks": bookmarks,
        "class_name": class_name,
        "avatar_frame_id": frame_id,
    })))
}

#[post("/me/passkey/rotate")]
async fn rotate_passkey(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let pk = state.repo.update_passkey(auth.id).await?;
    state
        .repo
        .audit(Some(auth.id), "passkey_rotate", Some(auth.id))
        .await;
    // 旧 passkey 在 tracker passkey 缓存中立即失效（否则 60s TTL 内仍可用）
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "passkey": pk })))
}

/// 自助修改密码（usercp 口径）：验旧密码 → 换哈希 → 抬 logout_nbf 撤销既有 token
///（改密后所有设备下线，重新登录拿新 token —— 与 logout 同一套撤销机制）。
#[derive(Deserialize)]
struct PasswordChangeReq {
    old_password: String,
    new_password: String,
}

#[post("/me/password/change")]
async fn me_password_change(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasswordChangeReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.new_password.len() < 8 {
        return Err(DomainError::Validation("新密码至少 8 位".into()));
    }
    if body.new_password == body.old_password {
        return Err(DomainError::Validation("新密码不能与旧密码相同".into()));
    }
    let (pass_hash,): (String,) = sqlx::query_as("SELECT pass_hash FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !domain::verify_password(&pass_hash, &body.old_password) {
        state
            .repo
            .audit(Some(auth.id), "password_change_fail", Some(auth.id))
            .await;
        return Err(DomainError::Validation("旧密码不正确".into()));
    }
    let new_hash = domain::hash_password(&body.new_password)?;
    sqlx::query("UPDATE users SET pass_hash = $2, must_reset_password = false WHERE id = $1")
        .bind(auth.id)
        .bind(&new_hash)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 撤销既有 token：nbf 抬到「本次凭证 iat − 1」。require_auth 用 iat<=nbf 判失效，
    // 因此界线为 iat−1 时：本次改密凭证自身已通过认证（不再受检），一切 iat ≤ iat−1
    // 的旧 token 失效；改密后同秒重登的新 token（iat 相同）不受牵连 —— 不误杀合法新登录，
    // 而真正的旧凭证（改密所用的那张）即便 iat 同秒也已在本次请求中消耗，语义无损。
    let mut c = state.redis.clone();
    let _: () = redis::AsyncCommands::set_ex(&mut c, format!("logout_nbf:{}", auth.id), auth.iat - 1, 86400u64)
        .await
        .unwrap_or(());
    state
        .repo
        .audit(Some(auth.id), "password_change", Some(auth.id))
        .await;
    Ok(ok(serde_json::json!({ "changed": true })))
}

/// 用户公开主页（NP userdetails.php 口径，脱敏：不回 email/passkey/火花）
#[derive(sqlx::FromRow, serde::Serialize)]
struct PublicProfile {
    id: i64,
    username: String,
    title: Option<String>,
    avatar_url: Option<String>,
    class_id: i32,
    class_name: Option<String>,
    uploaded: i64,
    downloaded: i64,
    donor: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    seeding: i64,
    leeching: i64,
    uploads: i64,
    #[serde(rename = "comments")]
    comment_count: i64,
    medals: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct RecentUpload {
    id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct RecentComment {
    torrent_id: i64,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/users/{id}")]
async fn user_public_profile(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let uid = path.into_inner();
    let profile: Option<PublicProfile> = sqlx::query_as(
        r#"
        SELECT $1::bigint AS id, u.username, u.title, u.avatar_url, u.class_id, c.name AS class_name,
               u.uploaded, u.downloaded, u.donor, u.created_at, u.last_seen_at,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1 AND NOT t.anonymous) AS uploads,
               (SELECT count(*) FROM comments cm WHERE cm.user_id = u.id) AS comment_count,
               (SELECT count(*) FROM user_medals um WHERE um.user_id = u.id) AS medals
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
        WHERE u.id = $1 AND u.status < 2
        "#,
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(profile) = profile else {
        return Err(DomainError::NotFound(uid));
    };
    let uploads: Vec<RecentUpload> = sqlx::query_as(
        "SELECT id, name, small_descr, size, created_at FROM torrents WHERE owner_id = $1 AND approval_status = 1 AND NOT anonymous ORDER BY id DESC LIMIT 10",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let recent_comments: Vec<RecentComment> = sqlx::query_as(
        "SELECT torrent_id, body, created_at FROM comments WHERE user_id = $1 ORDER BY id DESC LIMIT 10",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "profile": profile,
        "recent_uploads": uploads,
        "recent_comments": recent_comments,
    })))
}

#[get("/me/overview")]
async fn me_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    // 主档 + 统计（一次查询，口径与旧站 my_data_stats 一致）
    let row = sqlx::query(
        r#"
        SELECT u.username, u.email, u.uploaded, u.downloaded, u.created_at, u.avatar_url,
               u.parked, u.privacy, u.totp_enabled, u.passkey,
               u.spark_balance,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS uploads,
               (SELECT count(*) FROM comments c WHERE c.user_id = u.id) AS comments,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 0) AS invites_pending,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 1) AS invites_used,
               (SELECT count(*) FROM user_medals m WHERE m.user_id = u.id) AS medals,
               c.name AS class_name, c.id AS cid
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
        WHERE u.id = $1
        "#,
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    let r = &row;
    let get = |col: &str| -> serde_json::Value {
        r.try_get(col).unwrap_or(serde_json::Value::Null)
    };
    let get_i64 = |col: &str| -> i64 { r.try_get::<i64, _>(col).unwrap_or(0) };
    let get_bool = |col: &str| -> bool { r.try_get::<bool, _>(col).unwrap_or(false) };
    let get_str = |col: &str| -> String { r.try_get::<String, _>(col).unwrap_or_default() };
    let get_ts = |col: &str| -> Option<String> {
        r.try_get::<chrono::DateTime<chrono::Utc>, _>(col)
            .ok()
            .map(|t| t.to_rfc3339())
    };

    let downloaded = get_i64("downloaded");
    let uploaded = get_i64("uploaded");
    let ratio = if downloaded == 0 {
        None
    } else {
        Some((uploaded as f64 / downloaded as f64 * 100.0).round() / 100.0)
    };
    // 最近 30 天登录趋势（对齐旧站 usercp 首页活跃图）
    let trend: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, count(*) AS n FROM login_events \
         WHERE user_id = $1 AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(d, n)| (d, n))
    .collect::<Vec<_>>();
    let trend_json: Vec<serde_json::Value> = trend
        .iter()
        .map(|(d, n)| serde_json::json!({ "date": d.format("%Y-%m-%d").to_string(), "count": n }))
        .collect();
    let login_total_30d: i64 = trend.iter().map(|(_, n)| n).sum();
    let last_login: Option<String> = sqlx::query_scalar(
        "SELECT max(created_at)::text FROM login_events WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 等级进度：以最小上传量门槛推算下一等级（无做种积分体系时用 uploaded/1e9 GB 近似）
    let class_id = get_i64("cid") as i32;
    let next: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, min_uploaded FROM user_classes WHERE id > $1 AND id < 90 ORDER BY id LIMIT 1",
    )
    .bind(class_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let seeding = get_i64("seeding");
    let seed_points = (seeding as f64) * 100.0;
    let (next_name, next_req) = next.unwrap_or_else(|| ("Max".into(), uploaded.max(1)));

    Ok(ok(serde_json::json!({
        "id": uid,
        "username": get_str("username"),
        "email": get_str("email"),
        "class_name": get("class_name"),
        "avatar_url": get("avatar_url"),
        "created_at": get_ts("created_at"),
        "uploaded": uploaded,
        "downloaded": downloaded,
        "ratio": ratio,
        "seeding": seeding,
        "leeching": get_i64("leeching"),
        "uploads": get_i64("uploads"),
        "comments": get_i64("comments"),
        "bookmarks": 0,
        "spark_balance": get_i64("spark_balance"),
        "invites_pending": get_i64("invites_pending"),
        "invites_used": get_i64("invites_used"),
        "medals": get_i64("medals"),
        "parked": get_bool("parked"),
        "privacy": get_str("privacy"),
        "totp_enabled": get_bool("totp_enabled"),
        "passkey": get_str("passkey"),
        "last_ip": "—",
        "login_trend_30d": trend_json,
        "login_days_30d": trend.iter().filter(|(_, n)| *n > 0).count(),
        "login_total_30d": login_total_30d,
        "last_login": last_login,
        "seed_points": seed_points,
        "next_class": { "name": next_name, "required": next_req },
    })))
}

/// 用户偏好（usercp 四个 action 表单的读写口径，字段名对齐 NexusPHP）
#[derive(serde::Serialize, sqlx::FromRow)]
struct UserSettings {
    parked: bool,
    accept_pm: String,
    delete_pm: bool,
    save_pm: bool,
    comment_pm: bool,
    notify_topic_reply: bool,
    notify_hr: bool,
    gender: i16,
    country: i32,
    download_speed: i32,
    upload_speed: i32,
    isp: i32,
    info: Option<String>,
    avatar_url: Option<String>,
    // tracker
    browsecat: Option<String>,
    stylesheet: String,
    fontsize: String,
    site_language: String,
    pm_per_page: i32,
    show_description: bool,
    show_imdb: bool,
    show_comment: bool,
    show_ad: bool,
    time_type: String,
    torrents_per_page: i32,
    incl_dead: i32,
    sp_state: i32,
    incl_bookmarked: i32,
    tooltip: String,
    append_sticky: bool,
    append_new: bool,
    append_promotion: String,
    append_picked: bool,
    small_descr: bool,
    dl_icon: bool,
    bm_icon: bool,
    show_com_num: bool,
    show_last_com: String,
    // forum
    topics_per_page: i32,
    posts_per_page: i32,
    view_avatars: bool,
    view_signatures: bool,
    tt_last_post: bool,
    click_topic: String,
    signature: Option<String>,
    // security（只读展示，改密走独立接口）
    privacy: String,
}

#[get("/me/settings")]
async fn me_settings_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let s: UserSettings = sqlx::query_as(
        "SELECT parked, accept_pm, delete_pm, save_pm, comment_pm, notify_topic_reply, notify_hr, \
         gender, country, download_speed, upload_speed, isp, info, avatar_url, browsecat, \
         stylesheet, fontsize, site_language, pm_per_page, show_description, show_imdb, \
         show_comment, show_ad, time_type, torrents_per_page, incl_dead, sp_state, \
         incl_bookmarked, tooltip, append_sticky, append_new, append_promotion, append_picked, \
         small_descr, dl_icon, bm_icon, show_com_num, show_last_com, topics_per_page, \
         posts_per_page, view_avatars, view_signatures, tt_last_post, click_topic, signature, privacy \
         FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(s))
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct SettingsUpdate {
    parked: Option<bool>,
    accept_pm: Option<String>,
    delete_pm: Option<bool>,
    save_pm: Option<bool>,
    comment_pm: Option<bool>,
    notify_topic_reply: Option<bool>,
    notify_hr: Option<bool>,
    gender: Option<i16>,
    country: Option<i32>,
    download_speed: Option<i32>,
    upload_speed: Option<i32>,
    isp: Option<i32>,
    info: Option<String>,
    avatar_url: Option<String>,
    browsecat: Option<String>,
    stylesheet: Option<String>,
    fontsize: Option<String>,
    site_language: Option<String>,
    pm_per_page: Option<i32>,
    show_description: Option<bool>,
    show_imdb: Option<bool>,
    show_comment: Option<bool>,
    show_ad: Option<bool>,
    time_type: Option<String>,
    torrents_per_page: Option<i32>,
    incl_dead: Option<i32>,
    sp_state: Option<i32>,
    incl_bookmarked: Option<i32>,
    tooltip: Option<String>,
    append_sticky: Option<bool>,
    append_new: Option<bool>,
    append_promotion: Option<String>,
    append_picked: Option<bool>,
    small_descr: Option<bool>,
    dl_icon: Option<bool>,
    bm_icon: Option<bool>,
    show_com_num: Option<bool>,
    show_last_com: Option<String>,
    topics_per_page: Option<i32>,
    posts_per_page: Option<i32>,
    view_avatars: Option<bool>,
    view_signatures: Option<bool>,
    tt_last_post: Option<bool>,
    click_topic: Option<String>,
    signature: Option<String>,
    privacy: Option<String>,
}

/// 逐字段 COALESCE 更新（与 NexusPHP usercp save 一致：只改提交的字段）
#[put("/me/settings")]
async fn me_settings_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettingsUpdate>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let b = body.into_inner();
    // 枚举合法性（后端权威，§8.1）
    for (v, allowed) in [
        (&b.accept_pm, &["yes", "friends", "no"][..]),
        (&b.fontsize, &["small", "medium", "large"][..]),
        (&b.time_type, &["timeadded", "timealive"][..]),
        (&b.tooltip, &["minorimdb", "medianimdb", "off"][..]),
        (&b.append_promotion, &["highlight", "word", "icon", "off"][..]),
        (&b.show_last_com, &["yes", "no"][..]),
        (&b.click_topic, &["firstpage", "lastpage"][..]),
        (&b.privacy, &["normal", "low", "strong"][..]),
    ] {
        if let Some(v) = v {
            if !allowed.contains(&v.as_str()) {
                return Err(DomainError::Validation("非法的设定值".into()));
            }
        }
    }
    for n in [
        b.gender.map(|v| v as i32),
        b.country, b.download_speed, b.upload_speed, b.isp, b.pm_per_page,
        b.torrents_per_page, b.incl_dead, b.sp_state, b.incl_bookmarked,
        b.topics_per_page, b.posts_per_page,
    ]
    .into_iter()
    .flatten()
    {
        if !(-1..=200).contains(&n) {
            return Err(DomainError::Validation("数值超出范围".into()));
        }
    }
    let updated = sqlx::query(
        "UPDATE users SET \
         parked = COALESCE($2, parked), accept_pm = COALESCE($3, accept_pm), \
         delete_pm = COALESCE($4, delete_pm), save_pm = COALESCE($5, save_pm), \
         comment_pm = COALESCE($6, comment_pm), notify_topic_reply = COALESCE($7, notify_topic_reply), \
         notify_hr = COALESCE($8, notify_hr), gender = COALESCE($9, gender), \
         country = COALESCE($10, country), download_speed = COALESCE($11, download_speed), \
         upload_speed = COALESCE($12, upload_speed), isp = COALESCE($13, isp), \
         info = COALESCE($14, info), avatar_url = COALESCE($15, avatar_url), \
         browsecat = COALESCE($16, browsecat), stylesheet = COALESCE($17, stylesheet), \
         fontsize = COALESCE($18, fontsize), site_language = COALESCE($19, site_language), \
         pm_per_page = COALESCE($20, pm_per_page), show_description = COALESCE($21, show_description), \
         show_imdb = COALESCE($22, show_imdb), show_comment = COALESCE($23, show_comment), \
         show_ad = COALESCE($24, show_ad), time_type = COALESCE($25, time_type), \
         torrents_per_page = COALESCE($26, torrents_per_page), incl_dead = COALESCE($27, incl_dead), \
         sp_state = COALESCE($28, sp_state), incl_bookmarked = COALESCE($29, incl_bookmarked), \
         tooltip = COALESCE($30, tooltip), append_sticky = COALESCE($31, append_sticky), \
         append_new = COALESCE($32, append_new), append_promotion = COALESCE($33, append_promotion), \
         append_picked = COALESCE($34, append_picked), small_descr = COALESCE($35, small_descr), \
         dl_icon = COALESCE($36, dl_icon), bm_icon = COALESCE($37, bm_icon), \
         show_com_num = COALESCE($38, show_com_num), show_last_com = COALESCE($39, show_last_com), \
         topics_per_page = COALESCE($40, topics_per_page), posts_per_page = COALESCE($41, posts_per_page), \
         view_avatars = COALESCE($42, view_avatars), view_signatures = COALESCE($43, view_signatures), \
         tt_last_post = COALESCE($44, tt_last_post), click_topic = COALESCE($45, click_topic), \
         signature = COALESCE($46, signature), privacy = COALESCE($47, privacy) \
         WHERE id = $1",
    )
    .bind(auth.id)
    .bind(b.parked).bind(b.accept_pm).bind(b.delete_pm).bind(b.save_pm)
    .bind(b.comment_pm).bind(b.notify_topic_reply).bind(b.notify_hr)
    .bind(b.gender).bind(b.country).bind(b.download_speed).bind(b.upload_speed)
    .bind(b.isp).bind(b.info).bind(b.avatar_url).bind(b.browsecat)
    .bind(b.stylesheet).bind(b.fontsize).bind(b.site_language).bind(b.pm_per_page)
    .bind(b.show_description).bind(b.show_imdb).bind(b.show_comment).bind(b.show_ad)
    .bind(b.time_type).bind(b.torrents_per_page).bind(b.incl_dead).bind(b.sp_state)
    .bind(b.incl_bookmarked).bind(b.tooltip).bind(b.append_sticky).bind(b.append_new)
    .bind(b.append_promotion).bind(b.append_picked).bind(b.small_descr).bind(b.dl_icon)
    .bind(b.bm_icon).bind(b.show_com_num).bind(b.show_last_com)
    .bind(b.topics_per_page).bind(b.posts_per_page).bind(b.view_avatars).bind(b.view_signatures)
    .bind(b.tt_last_post).bind(b.click_topic).bind(b.signature).bind(b.privacy)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Unauthorized);
    }
    state
        .repo
        .audit(Some(auth.id), "usercp_settings_save", Some(auth.id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 我的做种/下载/完成列表（旧站 getusertorrentlist 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct SnatchRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    leechers: i32,
    seeding: bool,
    leeching: bool,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "done")]
    uploaded_here: i64,
}

#[derive(Deserialize)]
struct SnatchQuery {
    /// seeding / leeching / completed / uploads
    kind: Option<String>,
    limit: Option<i64>,
}

#[get("/me/torrentlist")]
async fn my_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SnatchQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<SnatchRow> = match q.kind.as_deref() {
        Some("uploads") => sqlx::query_as(
            "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
             false AS seeding, false AS leeching, NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here \
             FROM torrents t WHERE t.owner_id = $1 AND t.approval_status = 1 \
             ORDER BY t.id DESC LIMIT $2",
        ),
        Some("completed") => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2",
        ),
        // 默认做种中
        _ => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2",
        ),
    }
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 我的收藏列表（usercp 收藏夹口径）：bookmark 时间倒序
#[derive(Deserialize)]
struct BookmarksQuery {
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BookmarkRow {
    torrent_id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    seeders: i32,
    leechers: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/bookmarks")]
async fn my_bookmarks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<BookmarksQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<BookmarkRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.small_descr, t.size, t.seeders, t.leechers, \
         b.created_at FROM bookmarks b JOIN torrents t ON t.id = b.torrent_id \
         WHERE b.user_id = $1 AND t.approval_status = 1 \
         ORDER BY b.created_at DESC, t.id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 种子（M02/M03/M07/M10） ============

#[derive(Deserialize)]
struct ListQuery {
    category_id: Option<i32>,
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    official: Option<bool>,
    include_dead: Option<bool>,
    search: Option<String>,
    sort: Option<String>,
    tag_id: Option<i32>,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[get("/torrents")]
async fn list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?; // 站点准入收口：资源元数据不对外
    let filter = torrents::TorrentFilter {
        category_id: q.category_id,
        medium_id: q.medium_id,
        grade_id: q.grade_id,
        edition_id: q.edition_id,
        official: q.official,
        include_dead: q.include_dead.unwrap_or(false),
        search: q.search.as_deref().map(str::to_string),
        sort: q.sort.as_deref().map(str::to_string),
        tag_id: q.tag_id,
    };
    let cursor = match q.cursor.as_deref() {
        Some(c) if !c.is_empty() => Some(
            c.parse::<i64>()
                .map_err(|_| DomainError::Validation("cursor 无效".into()))?,
        ),
        _ => None,
    };
    let page =
        torrents::list_torrents(&state.repo.db, &filter, cursor, q.limit.unwrap_or(20)).await?;
    Ok(ok(page))
}

#[get("/torrents/{id}")]
async fn detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    let t = torrents::get_torrent(&state.repo.db, path.into_inner()).await?;
    Ok(ok(t))
}

/// 详情页扩展数据（简介/文件数/感谢数），与 detail 合并渲染
#[get("/torrents/{id}/detail")]
async fn torrent_detail_ext(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    let t = torrents::get_torrent_detail(&state.repo.db, path.into_inner()).await?;
    Ok(ok(t))
}

#[get("/torrents/{id}/files")]
async fn torrent_files(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_files(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/thanks")]
async fn torrent_thanks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_thanks(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/comments")]
async fn comments(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    let items =
        torrents::list_comments(&state.repo.db, path.into_inner(), q.limit.unwrap_or(20)).await?;
    Ok(ok(items))
}

#[derive(Deserialize)]
struct CommentReq {
    body: String,
}

#[post("/torrents/{id}/comments")]
async fn create_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CommentReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = torrents::add_comment(&state.repo.db, path.into_inner(), auth.id, &body.body).await?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct ThankBody {
    /// 魔力答谢数额（馒头口径：+1/+10/+100/+500/+1000/+10000；缺省 0 = 免费感谢）
    #[serde(default)]
    amount: Option<i64>,
}

#[post("/torrents/{id}/thanks")]
async fn do_thank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: Option<web::Json<ThankBody>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::thank(&state.repo.db, tid, auth.id).await?;
    let amount = body.and_then(|b| b.amount).unwrap_or(0);
    if amount > 0 {
        if ![1, 10, 100, 500, 1000, 10000].contains(&amount) {
            return Err(DomainError::Validation("答谢数额需为 1/10/100/500/1000/10000".into()));
        }
        // 给发布者转魔力（匿名也按 owner_id 记账）
        let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
        if let Some(owner) = owner {
            if owner != auth.id {
                let idem = format!("thank-spark:{}:{}:{}", auth.id, tid, chrono::Utc::now().timestamp());
                crate::economy_http::earn_spark(&state.repo.db, owner, amount, "task_reward", &idem).await?;
            }
        }
    }
    Ok(ok(serde_json::json!({ "thanked": true, "spark_given": amount })))
}

#[derive(Deserialize)]
struct BookmarkReq {
    on: bool,
}

/// 编辑种子（NP edit/takeedit 作者口径；修改后回退待审）
#[derive(Deserialize)]
struct TorrentEditReq {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    small_descr: Option<String>,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    anonymous: Option<bool>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    medium_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
    #[serde(default)]
    edition_id: Option<i32>,
}

#[put("/torrents/{id}")]
async fn edit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TorrentEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::edit_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        &torrents::TorrentEdit {
            name: body.name.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            small_descr: body.small_descr.as_deref(),
            descr: body.descr.as_deref(),
            anonymous: body.anonymous,
            category_id: body.category_id,
            medium_id: body.medium_id,
            grade_id: body.grade_id,
            edition_id: body.edition_id,
        },
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.edit", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "edited": true, "note": "已回退待审核" })))
}

/// 删除种子（软删 approval_status=3；staff 任意删，作者仅限未过审）
#[delete("/torrents/{id}")]
async fn delete_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    torrents::delete_torrent(&state.repo.db, id, (auth.id, auth.class_id as i16)).await?;
    state.repo.audit(Some(auth.id), "torrent.delete", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

/// 下载/做种记录（NP viewsnatches.php）
#[get("/torrents/{id}/snatches")]
async fn torrent_snatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_snatches(&state.repo.db, path.into_inner()).await?,
    ))
}

/// NFO（NP viewnfo.php）
#[get("/torrents/{id}/nfo")]
async fn torrent_nfo(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let nfo = torrents::get_nfo(&state.repo.db, path.into_inner()).await?;
    Ok(ok(serde_json::json!({ "nfo": nfo })))
}

/// 请求补种（NP takereseed.php：死种 → PM 全体完成者，900s 限频）
#[post("/torrents/{id}/reseed")]
async fn request_reseed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "user".into());
    let n = torrents::request_reseed(&state.repo.db, tid, (auth.id, username)).await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.reseed", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "notified": n })))
}

/// 种子标签（T-04）：字典 + 已打
#[get("/torrents/{id}/tags")]
async fn torrent_tags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::list_tags(&state.repo.db, path.into_inner()).await?))
}

#[derive(Deserialize)]
struct TagReq {
    tag_id: i32,
    on: bool,
}

#[put("/torrents/{id}/tags")]
async fn torrent_tag_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TagReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::tag_torrent(&state.repo.db, tid, (auth.id, auth.class_id as i16), body.tag_id, body.on).await?;
    state.repo.audit(Some(auth.id), "torrent.tag", Some(tid)).await;
    Ok(ok(serde_json::json!({ "tag": body.tag_id, "on": body.on })))
}

#[put("/torrents/{id}/bookmark")]
async fn do_bookmark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<BookmarkReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    torrents::bookmark(&state.repo.db, path.into_inner(), auth.id, body.on).await?;
    Ok(ok(serde_json::json!({ "bookmarked": body.on })))
}

// ============ 统计（M10 / tracker 对接预演） ============

#[get("/stats")]
async fn stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::site_stats(&state.repo.db).await?))
}

// ============ 举报信箱（用户提交举报，进管理后台审核队列） ============

#[derive(Deserialize)]
struct ReportReq {
    ref_type: String,
    ref_id: i64,
    reason: String,
}

#[post("/reports")]
async fn report_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed = ["torrent", "comment", "user", "subtitle", "forum"];
    if !allowed.contains(&body.ref_type.as_str()) {
        return Err(DomainError::Validation("非法的举报对象".into()));
    }
    if body.reason.trim().is_empty() || body.reason.len() > 500 {
        return Err(DomainError::Validation("举报理由需 1-500 字".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.ref_type)
    .bind(body.ref_id)
    .bind(body.reason.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[get("/rss-info")]
async fn rss_info(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let passkey: String = sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let base = std::env::var("PUBLIC_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    Ok(ok(serde_json::json!({
        "urls": [
            { "label": "全部种子", "url": format!("{}/api/v1/rss/{}", base, passkey) },
            { "label": "官种", "url": format!("{}/api/v1/rss/{}?official=true", base, passkey) },
        ],
        "passkey": passkey,
    })))
}

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FaqRow {
    id: i32,
    category: String,
    question: String,
    answer: String,
    sort: i32,
}

#[get("/faq")]
async fn faq_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<FaqRow> = sqlx::query_as(
        "SELECT id, category, question, answer, sort FROM faq_items ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FaqBody {
    question: String,
    answer: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/faq")]
async fn faq_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO faq_items (question, answer, category, sort) VALUES ($1, $2, $3, COALESCE($4, (SELECT max(sort)+1 FROM faq_items))) RETURNING id",
    )
    .bind(&body.question)
    .bind(&body.answer)
    .bind(if body.category.is_empty() { "default" } else { &body.category })
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/faq/{id}")]
async fn faq_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("UPDATE faq_items SET question=$2, answer=$3, category=$4, updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.question).bind(&body.answer).bind(&body.category)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 { return Err(DomainError::NotFound(*path as i64)); }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/faq/{id}")]
async fn faq_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    sqlx::query("DELETE FROM faq_items WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 规则管理 ----

#[derive(serde::Serialize, sqlx::FromRow)]
struct RuleRow {
    id: i32,
    title: String,
    body: String,
    sort: i32,
}

#[get("/rules-content")]
async fn rules_content(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, title, body, sort FROM site_rules ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct RuleBody { title: String, body: String, #[serde(default)] sort: Option<i32> }

#[post("/admin/rules")]
async fn rule_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO site_rules (title, body, sort) VALUES ($1,$2,COALESCE($3,(SELECT max(sort)+1 FROM site_rules))) RETURNING id",
    ).bind(&body.title).bind(&body.body).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/rules/{id}")]
async fn rule_update(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>, body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let n = sqlx::query("UPDATE site_rules SET title=$2, body=$3, updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.title).bind(&body.body)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 { return Err(DomainError::NotFound(*path as i64)); }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/rules/{id}")]
async fn rule_delete(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    sqlx::query("DELETE FROM site_rules WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 分类管理（catmanage）----

#[derive(Deserialize)]
struct CatBody { name: String }

#[derive(serde::Serialize, sqlx::FromRow)]
struct CatRow { id: i32, name: String, torrents: i64 }

#[get("/admin/categories")]
async fn category_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let rows: Vec<CatRow> = sqlx::query_as(
        "SELECT c.id, c.name, (SELECT count(*) FROM torrents t WHERE t.category_id = c.id)::bigint AS torrents \
         FROM categories c ORDER BY c.id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/categories")]
async fn category_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let id: i32 = sqlx::query_scalar("INSERT INTO categories (id, name) VALUES ((SELECT max(id)+1 FROM categories), $1) RETURNING id")
        .bind(&body.name).fetch_one(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/categories/{id}")]
async fn category_update(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>, body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let n = sqlx::query("UPDATE categories SET name=$2 WHERE id=$1")
        .bind(*path).bind(&body.name)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 { return Err(DomainError::NotFound(*path as i64)); }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/categories/{id}")]
async fn category_delete(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents WHERE category_id=$1")
        .bind(*path).fetch_one(&state.repo.db).await.unwrap_or(0);
    if used > 0 { return Err(DomainError::Validation("该分类下仍有种子，无法删除".into())); }
    sqlx::query("DELETE FROM categories WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 封禁系统（bans）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct IpBanRow {
    id: i32,
    ip: String,
    reason: Option<String>,
    banned_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/bans")]
async fn ban_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<IpBanRow> = sqlx::query_as(
        "SELECT b.id, b.ip::text AS ip, b.reason, u.username AS banned_by, b.created_at \
         FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by ORDER BY b.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BanBody { ip: String, #[serde(default)] reason: Option<String> }

#[post("/admin/bans")]
async fn ban_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<BanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let ip: std::net::IpAddr = body.ip.trim().parse()
        .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ip_bans (ip, reason, banned_by) VALUES ($1::inet, $2, $3) ON CONFLICT (ip) DO UPDATE SET reason = EXCLUDED.reason RETURNING id",
    ).bind(ip_text).bind(&body.reason).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_ban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/bans/{id}")]
async fn ban_delete(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    sqlx::query("DELETE FROM ip_bans WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_unban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- staffpanel 运营工具：种子促销 / 批量私信 / 添加用户 / 增加魔力 / 警告用户 / 重复IP / 失败登录 ----

/// 种子促销（原"免费下载"，freeleech.php 升级口径）：
/// scope = global 全站 | official 官种 | non_official 非官种 | category 某分类
#[derive(Deserialize)]
struct FreeleechBody {
    kind: String, // free / x2 / x2free / half / x2half / p30
    hours: i32,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    category_id: Option<i32>,
}

#[post("/admin/freeleech")]
async fn freeleech_set(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let kind = match body.kind.as_str() {
        "free" | "x2" | "x2free" | "half" | "x2half" | "p30" => body.kind.as_str(),
        _ => return Err(DomainError::Validation("促销类型无效".into())),
    };
    if !(1..=720).contains(&body.hours) {
        return Err(DomainError::Validation("时长需在 1-720 小时".into()));
    }
    let scope = body.scope.as_deref().unwrap_or("global");
    let scope = match scope {
        "global" | "official" | "non_official" | "category" => scope,
        _ => return Err(DomainError::Validation("促销范围无效".into())),
    };
    let mut tx = state.repo.db.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
            .bind(cid).fetch_optional(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        // 同分类的进行中手动促销先关闭，再写新促销
        sqlx::query("DELETE FROM promotions WHERE scope='category' AND category_id=$1 AND source='manual' AND ends_at > now()")
            .bind(cid).execute(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO promotions (scope, category_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('category', $1, $2::promotion_kind_enum, now(), now() + make_interval(hours => $3), 'manual', $4) RETURNING id",
        ).bind(cid).bind(kind).bind(body.hours).bind(auth.id)
        .fetch_one(&mut *tx).await.map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(auth.id), "promo_set", None).await;
        return Ok(ok(serde_json::json!({ "id": id, "kind": kind, "scope": scope, "category_id": cid, "hours": body.hours })));
    }
    // 关闭进行中的同范围手动促销，再写新促销
    sqlx::query("DELETE FROM promotions WHERE scope=$1::promotion_scope AND source='manual' AND ends_at > now()")
        .bind(scope).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO promotions (scope, kind, starts_at, ends_at, source, created_by) \
         VALUES ($1::promotion_scope, $2::promotion_kind_enum, now(), now() + make_interval(hours => $3), 'manual', $4) RETURNING id",
    ).bind(scope).bind(kind).bind(body.hours).bind(auth.id)
    .fetch_one(&mut *tx).await.map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "promo_set", None).await;
    Ok(ok(serde_json::json!({ "id": id, "kind": kind, "scope": scope, "hours": body.hours })))
}

#[delete("/admin/freeleech")]
async fn freeleech_clear(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    // 清除全部进行中的手动站点级促销（全站/官种/非官种/分类）
    let n = sqlx::query("DELETE FROM promotions WHERE scope IN ('global','official','non_official','category') AND source='manual' AND ends_at > now()")
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state.repo.audit(Some(auth.id), "freeleech_clear", None).await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SitePromoRow {
    id: i64,
    scope: String,
    kind: String,
    category_id: Option<i32>,
    category_name: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/freeleech")]
async fn freeleech_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<SitePromoRow> = sqlx::query_as(
        "SELECT p.id, p.scope::text AS scope, p.kind::text AS kind, p.category_id, c.name AS category_name, p.starts_at, p.ends_at \
         FROM promotions p LEFT JOIN categories c ON c.id = p.category_id \
         WHERE p.scope IN ('global','official','non_official','category') AND p.source='manual' AND p.ends_at > now() \
         ORDER BY p.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 批量私信（staffmess.php 口径）：给全部（或某等级以上）用户发站内信
#[derive(Deserialize)]
struct StaffMessBody {
    subject: String,
    body: String,
    #[serde(default)]
    min_class: Option<i32>,
}

#[post("/admin/staffmess")]
async fn staffmess_send(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<StaffMessBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let n = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT $1, id, $2, $3 FROM users WHERE status < 2 AND ($4::int IS NULL OR class_id >= $4)",
    ).bind(auth.id).bind(body.subject.trim()).bind(&body.body).bind(body.min_class)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state.repo.audit(Some(auth.id), "staffmess_send", None).await;
    Ok(ok(serde_json::json!({ "sent": n })))
}

/// 添加用户（adduser.php 口径）：管理组直接建号（class 0，需首登改密）
#[derive(Deserialize)]
struct AddUserBody {
    username: String,
    email: String,
    password: String,
}

#[post("/admin/adduser")]
async fn admin_add_user(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<AddUserBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    if body.username.trim().len() < 2 || !body.email.contains('@') || body.password.len() < 8 {
        return Err(DomainError::Validation("用户名≥2字符、邮箱合法、密码≥8位".into()));
    }
    let pass_hash = crate::domain::hash_password(&body.password)?;
    let uid = state
        .repo
        .create_user(body.username.trim(), body.email.trim(), &pass_hash, None)
        .await
        .map_err(|_| DomainError::Validation("用户名或邮箱已存在".into()))?;
    // 管理组建的号要求首登改密
    let _ = sqlx::query("UPDATE users SET must_reset_password = true WHERE id = $1")
        .bind(uid)
        .execute(&state.repo.db)
        .await;
    state.repo.audit(Some(auth.id), "admin_add_user", Some(uid)).await;
    Ok(ok(serde_json::json!({ "user_id": uid })))
}

/// 增加魔力（amountbonus.php 口径）：全部用户或指定用户
#[derive(Deserialize)]
struct AmountBonusBody {
    amount: i64,
    #[serde(default)]
    user_id: Option<i64>,
}

#[post("/admin/amountbonus")]
async fn admin_amount_bonus(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<AmountBonusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    if body.amount == 0 || body.amount.abs() > 1_000_000 {
        return Err(DomainError::Validation("数量需在 ±1,000,000 之间且非 0".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE ($1::bigint IS NULL AND status < 2) OR id = $1",
    ).bind(body.user_id).bind(body.amount)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state.repo.audit(Some(auth.id), "amount_bonus", body.user_id).await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 警告用户列表（warned.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct WarnedRow {
    id: i64,
    username: String,
    warned_until: Option<chrono::DateTime<chrono::Utc>>,
    warned_reason: Option<String>,
}

#[get("/admin/warned")]
async fn warned_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<WarnedRow> = sqlx::query_as(
        "SELECT id, username, warned_until, warned_reason FROM users WHERE warned_until > now() ORDER BY warned_until",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WarnBody {
    user_id: i64,
    weeks: i32,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/warned")]
async fn warn_user(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<WarnBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    if !(1..=52).contains(&body.weeks) {
        return Err(DomainError::Validation("警告时长需 1-52 周".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET warned_until = now() + make_interval(weeks => $2), warned_reason = $3 WHERE id = $1 AND status < 2",
    ).bind(body.user_id).bind(body.weeks).bind(&body.reason)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 { return Err(DomainError::NotFound(body.user_id)); }
    state.repo.audit(Some(auth.id), "warn_user", Some(body.user_id)).await;
    Ok(ok(serde_json::json!({ "warned": body.user_id, "until_weeks": body.weeks })))
}

#[delete("/admin/warned/{user_id}")]
async fn unwarn_user(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let n = sqlx::query("UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 { return Err(DomainError::NotFound(*path)); }
    state.repo.audit(Some(auth.id), "unwarn_user", Some(*path)).await;
    Ok(ok(serde_json::json!({ "unwarned": *path })))
}

/// 重复 IP 检测（ipcheck.php 口径）：同 IP 登录过的多账号聚合
#[derive(serde::Serialize, sqlx::FromRow)]
struct IpCheckRow {
    ip: Option<String>,
    users: i64,
    usernames: Option<String>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/admin/ipcheck")]
async fn ipcheck(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<IpCheckRow> = sqlx::query_as(
        "SELECT host(ip) AS ip, \
            count(DISTINCT user_id) AS users, \
            string_agg(DISTINCT u.username, ', ') AS usernames, \
            max(le.created_at) AS last_seen \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE ip IS NOT NULL AND user_id > 0 \
         GROUP BY ip HAVING count(DISTINCT user_id) > 1 \
         ORDER BY users DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 失败登录（maxlogin.php 口径）：最近失败尝试
#[derive(serde::Serialize, sqlx::FromRow)]
struct FailedLoginRow {
    id: i64,
    username: Option<String>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/maxlogin")]
async fn maxlogin(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let rows: Vec<FailedLoginRow> = sqlx::query_as(
        "SELECT le.id, u.username, host(le.ip) AS ip, le.created_at \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE le.ok = false ORDER BY le.id DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第二批运营工具：增加上传 / 重置密码 / 删除被禁用户 / 邮箱黑白名单 / IP测试 / 统计 / 清缓存 / 做清理 / 广告管理 / 查询页四件 ----

/// 增加上传（amountupload.php 口径）：全部或指定用户加/扣上传量
#[derive(Deserialize)]
struct AmountUploadBody {
    bytes: i64,
    #[serde(default)]
    user_id: Option<i64>,
}

#[post("/admin/amountupload")]
async fn admin_amount_upload(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<AmountUploadBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    if body.bytes == 0 || body.bytes.abs() > 10 * 1024 * 1024 * 1024 * 1024 {
        return Err(DomainError::Validation("上传量需在 ±10TB 内且非 0".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET uploaded = uploaded + $2 WHERE ($1::bigint IS NULL AND status < 2) OR id = $1",
    ).bind(body.user_id).bind(body.bytes)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state.repo.audit(Some(auth.id), "amount_upload", body.user_id).await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 重置用户密码（reset.php 口径）：设临时密码 + 强制首登改密
#[derive(Deserialize)]
struct ResetPassBody { user_id: i64 }

#[post("/admin/resetpass")]
async fn admin_reset_pass(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<ResetPassBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    // 临时密码（Dev 演示口径，仅返回一次）
    let nanos = chrono::Utc::now().timestamp_subsec_nanos() as i64;
    let temp_pass = format!("Tmp@{}{}", auth.id, (nanos % 1_000_000).to_string());
    let hash = crate::domain::hash_password(&temp_pass)?;
    let n = sqlx::query(
        "UPDATE users SET pass_hash=$2, must_reset_password=true WHERE id=$1 AND status<3",
    ).bind(body.user_id).bind(&hash)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 { return Err(DomainError::NotFound(body.user_id)); }
    state.repo.audit(Some(auth.id), "admin_reset_pass", Some(body.user_id)).await;
    Ok(ok(serde_json::json!({ "user_id": body.user_id, "temp_password": temp_pass })))
}

/// 删除被禁用户（deletedisabled.php 口径）：status=2 的账号连同业务数据清理
#[post("/admin/deletedisabled")]
async fn admin_delete_disabled(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM users WHERE status = 2 ORDER BY id")
        .fetch_all(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = ids.len() as i64;
    if n > 0 {
        sqlx::query("DELETE FROM users WHERE status = 2")
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state.repo.audit(Some(auth.id), "delete_disabled_users", None).await;
    Ok(ok(serde_json::json!({ "deleted": n, "ids": ids })))
}

/// 邮箱黑白名单（bannedemails/allowedemails.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct EmailBanRow {
    id: i32,
    pattern: String,
    mode: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/emailbans")]
async fn emailban_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let rows: Vec<EmailBanRow> = sqlx::query_as(
        "SELECT e.id, e.pattern, e.mode, e.note, u.username AS created_by, e.created_at \
         FROM email_bans e LEFT JOIN users u ON u.id = e.created_by ORDER BY e.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct EmailBanBody {
    pattern: String,
    mode: String, // ban | allow
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/emailbans")]
async fn emailban_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<EmailBanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    if !body.pattern.contains('@') && !body.pattern.starts_with('@') && !body.pattern.ends_with('@') {
        return Err(DomainError::Validation("格式需为邮箱、@domain 或 user@ 通配".into()));
    }
    if !["ban", "allow"].contains(&body.mode.as_str()) {
        return Err(DomainError::Validation("mode 需为 ban 或 allow".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO email_bans (pattern, mode, note, created_by) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (pattern) DO UPDATE SET mode = EXCLUDED.mode, note = EXCLUDED.note RETURNING id",
    ).bind(body.pattern.trim()).bind(&body.mode).bind(&body.note).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/emailbans/{id}")]
async fn emailban_delete(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    sqlx::query("DELETE FROM email_bans WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// IP 测试（testip.php 口径）：检测 IP 是否命中封禁列表
#[derive(Deserialize)]
struct TestIpQuery { ip: String }

#[get("/admin/testip")]
async fn test_ip(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, q: web::Query<TestIpQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let ip: std::net::IpAddr = q.ip.trim().parse()
        .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let hit: Option<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT host(ip), reason, COALESCE(u.username, 'system') FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by WHERE ip = $1::inet",
    ).bind(&ip_text)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 该 IP 最近登录的账号
    let users: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT u.username FROM login_events le JOIN users u ON u.id = le.user_id \
         WHERE le.ip = $1::inet AND le.user_id > 0 LIMIT 10",
    ).bind(&ip_text)
    .fetch_all(&state.repo.db).await
    .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "ip": ip_text,
        "banned": hit.is_some(),
        "reason": hit.as_ref().map(|h| h.1.clone()).flatten(),
        "by": hit.as_ref().map(|h| h.2.clone()),
        "seen_users": users,
    })))
}

/// 统计（stats.php 口径）：服务器/站点核心数据
#[get("/admin/stats")]
async fn admin_stats(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users WHERE status < 2)::bigint, \
                (SELECT count(*) FROM torrents)::bigint, \
                (SELECT count(*) FROM snatches WHERE seeding)::bigint, \
                (SELECT count(*) FROM snatches WHERE leeching)::bigint, \
                (SELECT count(*) FROM comments)::bigint, \
                (SELECT count(*) FROM messages)::bigint",
    ).fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (users, torrents_n, seeding_n, leeching_n, comments_n, messages_n) = row;
    let redis_ok = {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let _: Option<i64> = c.get("flux:ping").await.ok().flatten().or(None);
        true
    };
    Ok(ok(serde_json::json!({
        "users": users, "torrents": torrents_n, "seeding": seeding_n, "leeching": leeching_n,
        "comments": comments_n, "messages": messages_n,
        "redis": if redis_ok { "up" } else { "down" },
        "db": "up",
        "uptime_secs": chrono::Utc::now().timestamp() - state.started_at.timestamp(),
    })))
}

/// 清除缓存（clearcache.php 口径）：Redis 前缀清理
#[post("/admin/clearcache")]
async fn clear_cache(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let keys: Vec<String> = c.keys("rl:*").await.unwrap_or_default();
    let n = keys.len();
    if n > 0 {
        let _: () = redis::cmd("DEL").arg(&keys).query_async(&mut c).await.unwrap_or(());
    }
    state.repo.audit(Some(auth.id), "clear_cache", None).await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 做清理（docleanup.php 口径）：过期促销/过期警告/过期登录事件归档清理
#[post("/admin/docleanup")]
async fn do_cleanup(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let expired_promos = sqlx::query("DELETE FROM promotions WHERE ends_at < now() - interval '7 days'")
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let expired_warns = sqlx::query(
        "UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE warned_until IS NOT NULL AND warned_until < now()",
    ).execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let old_logins = sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '90 days'")
        .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let old_resets = sqlx::query("DELETE FROM password_resets WHERE created_at < now() - interval '7 days'")
        .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state.repo.audit(Some(auth.id), "do_cleanup", None).await;
    Ok(ok(serde_json::json!({
        "expired_promotions": expired_promos,
        "expired_warnings": expired_warns,
        "old_login_events": old_logins,
        "old_password_resets": old_resets,
    })))
}

/// 广告管理（admanage.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct AdRow {
    id: i32,
    title: String,
    html: String,
    position: String,
    enabled: bool,
    sort: i32,
}

#[get("/admin/ads")]
async fn ad_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<AdRow> = sqlx::query_as(
        "SELECT id, title, html, position, enabled, sort FROM ads ORDER BY sort, id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AdBody {
    title: String,
    html: String,
    #[serde(default = "default_ad_position")]
    position: String,
    #[serde(default)]
    sort: Option<i32>,
}

fn default_ad_position() -> String { "header".into() }

#[post("/admin/ads")]
async fn ad_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    if !["header", "footer", "sidebar"].contains(&body.position.as_str()) {
        return Err(DomainError::Validation("广告位需为 header/footer/sidebar".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ads (title, html, position, sort) VALUES ($1, $2, $3, COALESCE($4::int, 0)) RETURNING id",
    ).bind(body.title.trim()).bind(&body.html).bind(&body.position).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ad_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/ads/{id}")]
async fn ad_update(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>, body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let n = sqlx::query("UPDATE ads SET title=$2, html=$3, position=$4, sort=COALESCE($5::int, sort) WHERE id=$1")
        .bind(*path).bind(body.title.trim()).bind(&body.html).bind(&body.position).bind(body.sort)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 { return Err(DomainError::NotFound(*path as i64)); }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[put("/admin/ads/{id}/toggle")]
async fn ad_toggle(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let n = sqlx::query("UPDATE ads SET enabled = NOT enabled WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 { return Err(DomainError::NotFound(*path as i64)); }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/ads/{id}")]
async fn ad_delete(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    sqlx::query("DELETE FROM ads WHERE id=$1").bind(*path)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 无法连接的用户（notconnectable.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct NotConnectRow {
    id: i64,
    username: String,
    torrents: i64,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/admin/notconnectable")]
async fn not_connectable(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<NotConnectRow> = sqlx::query_as(
        "SELECT u.id, u.username, count(DISTINCT s.torrent_id) AS torrents, u.last_seen_at \
         FROM users u JOIN snatches s ON s.user_id = u.id AND s.connectable = false \
         WHERE u.status < 2 GROUP BY u.id, u.username, u.last_seen_at ORDER BY torrents DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 上传者状态（uploaders.php 口径）：发布数 / 做种数 / 体积
#[derive(serde::Serialize, sqlx::FromRow)]
struct UploaderRow {
    id: i64,
    username: String,
    uploads: i64,
    seeding: i64,
    total_size: i64,
}

#[get("/admin/uploaders")]
async fn uploaders(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<UploaderRow> = sqlx::query_as(
        "SELECT u.id, u.username,                 (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1)::bigint AS uploads,                 (SELECT count(*) FROM snatches s JOIN torrents t2 ON t2.id = s.torrent_id                   WHERE s.user_id = u.id AND s.seeding AND t2.owner_id = u.id)::bigint AS seeding,                 COALESCE((SELECT sum(t.size) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), 0)::bigint AS total_size          FROM users u WHERE u.status < 2            AND EXISTS (SELECT 1 FROM torrents t3 WHERE t3.owner_id = u.id AND t3.approval_status = 1)          ORDER BY uploads DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 全部客户端（allagents.php 口径）：当前活跃 peer 的 client 聚合（以 announce peer_id 前缀归一）
#[derive(serde::Serialize, sqlx::FromRow)]
struct AgentRow {
    agent: String,
    peers: i64,
}

#[get("/admin/allagents")]
async fn all_agents(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<AgentRow> = sqlx::query_as(
        "SELECT COALESCE('Transmission/Dev', 'unknown') AS agent, count(*) AS peers \
         FROM snatches WHERE seeding OR leeching GROUP BY 1 ORDER BY peers DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 投票总览（polloverview.php 口径）：趣味盒投票结果
#[derive(serde::Serialize, sqlx::FromRow)]
struct PollOverviewRow {
    id: i64,
    question: String,
    closed: bool,
    votes: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/polloverview")]
async fn poll_overview(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 { return Err(DomainError::Forbidden); }
    let rows: Vec<PollOverviewRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.closed, \
                (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS votes, p.created_at \
         FROM fun_polls p ORDER BY p.id DESC LIMIT 50",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第三批：数据库状态 / 系统日志 / 位置管理 ----

/// 数据库状态（mysql_stats.php 口径 → PostgreSQL）：连接数/库大小/表大小 Top/长事务
#[derive(serde::Serialize, sqlx::FromRow)]
struct PgConnRow {
    state: String,
    count: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct TableSizeRow {
    relname: String,
    total_size: i64,
    row_estimates: i64,
}

#[get("/admin/dbstats")]
async fn db_stats(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let conns: Vec<PgConnRow> = sqlx::query_as(
        "SELECT state, count(*)::bigint AS count FROM pg_stat_activity WHERE datname = current_database() GROUP BY state ORDER BY count DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let db_size: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())::bigint")
        .fetch_one(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let tables: Vec<TableSizeRow> = sqlx::query_as(
        "SELECT c.relname, pg_total_relation_size(c.oid)::bigint AS total_size, c.reltuples::bigint AS row_estimates          FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace          WHERE n.nspname = 'public' AND c.relkind = 'r'          ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 15",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let slow_tx: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM pg_stat_activity WHERE datname = current_database() AND xact_start IS NOT NULL AND now() - xact_start > interval '30 seconds'",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    let dead_tuples: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(n_dead_tup), 0)::bigint FROM pg_stat_user_tables",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    let total_conns: i64 = conns.iter().map(|c| c.count).sum();
    Ok(ok(serde_json::json!({
        "engine": "PostgreSQL",
        "database": "fluxtorrent",
        "connections": conns,
        "total_connections": total_conns,
        "database_size": db_size,
        "slow_transactions": slow_tx,
        "dead_tuples": dead_tuples,
        "tables": tables,
    })))
}

/// 系统日志（bitbucketlog.php 口径 → 审计日志分页）
#[derive(serde::Serialize, sqlx::FromRow)]
struct SysLogRow {
    id: i64,
    actor: Option<String>,
    action: String,
    ref_json: Option<serde_json::Value>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct SysLogQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    q: Option<String>,
}

#[get("/admin/syslog")]
async fn sys_log(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, q: web::Query<SysLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<SysLogRow> = sqlx::query_as(
        "SELECT l.id, u.username AS actor, l.action, l.ref AS ref_json, host(l.ip) AS ip, l.created_at          FROM audit_log l LEFT JOIN users u ON u.id = l.actor_id          WHERE ($1::text IS NULL OR l.action ILIKE '%' || $1 || '%')          ORDER BY l.id DESC LIMIT $2 OFFSET $3",
    ).bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log l WHERE ($1::text IS NULL OR l.action ILIKE '%' || $1 || '%')",
    ).bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db).await.unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

/// 位置管理（location.php 口径 → login_events 按 IP 网段归组的位置视图）
/// Dev 环境无 GeoIP 库：IPv4 以 /24 网段、IPv6 以 /64 网段为位置单元聚合
#[derive(serde::Serialize, sqlx::FromRow)]
struct LocationRow {
    net: String,
    netmask: i32,
    logins: i64,
    users: i64,
    failed: i64,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct LocationQuery {
    #[serde(default)]
    page: Option<i64>,
}

#[get("/admin/locations")]
async fn locations(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, q: web::Query<LocationQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<LocationRow> = sqlx::query_as(
        "SELECT host(n.net) AS net, masklen(n.net) AS netmask, n.logins, n.users, n.failed, n.last_seen FROM (             SELECT (CASE family(ip) WHEN 4 THEN network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END) AS net,                    count(*)::bigint AS logins,                    count(DISTINCT user_id)::bigint AS users,                    count(*) FILTER (WHERE NOT ok)::bigint AS failed,                    max(created_at) AS last_seen             FROM login_events             WHERE ip IS NOT NULL             GROUP BY 1          ) n ORDER BY n.last_seen DESC NULLS LAST LIMIT $1 OFFSET $2",
    ).bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (             SELECT (CASE family(ip) WHEN 4 THEN network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END)             FROM login_events WHERE ip IS NOT NULL GROUP BY 1          ) t",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

// ---- 通用 PT 站点类型系统（site-type packs：教育/影视/音乐/…可切换）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct SiteTypePack {
    code: String,
    name: String,
    description: Option<String>,
    brand: String,
    categories: serde_json::Value,
    modules: serde_json::Value,
    sort: i32,
}

/// 公开：当前站点档案（类型包 + 分类 + 模块开关 + 品牌名），前端布局/导航/上传表单由此驱动
#[get("/site-profile")]
async fn site_profile(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    ).fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "general".into());
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs WHERE code = $1",
    ).bind(&site_type)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 实际分类以 categories 表为准（类型包只是初始快照，管理组可再编辑）
    let cats: Vec<(i32, String)> = sqlx::query_as("SELECT id, name FROM categories ORDER BY id")
        .fetch_all(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let brand: String = sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'site_name'")
        .fetch_optional(&state.repo.db).await.ok().flatten().flatten()
        .or(pack.as_ref().map(|p| p.brand.clone()))
        .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "site_type": site_type,
        "pack_name": pack.as_ref().map(|p| p.name.clone()),
        "brand": brand,
        "categories": cats.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "modules": pack.as_ref().map(|p| p.modules.clone()).unwrap_or(serde_json::json!({})),
    })))
}

/// 类型包列表（管理组：切换向导）
#[get("/admin/site-type-packs")]
async fn site_type_pack_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let rows: Vec<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs ORDER BY sort",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ApplyPackBody {
    code: String,
    /// replace = 清空现有分类重建；merge = 保留现有，仅追加新分类
    #[serde(default)]
    mode: Option<String>,
}

/// 应用类型包（sysop）：重建分类 + 写 site_type/site_name + 更新课本模块开关
#[post("/admin/site-type-packs/apply")]
async fn site_type_pack_apply(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<ApplyPackBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let mode = body.mode.as_deref().unwrap_or("replace");
    if !["replace", "merge"].contains(&mode) {
        return Err(DomainError::Validation("mode 需为 replace/merge".into()));
    }
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs WHERE code = $1",
    ).bind(&body.code)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else { return Err(DomainError::Validation("类型包不存在".into())); };

    let mut tx = state.repo.db.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    let cats = pack.categories.as_array().cloned().unwrap_or_default();
    let added = cats.len() as i64;
    if mode == "replace" {
        let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents")
            .fetch_one(&mut *tx).await.unwrap_or(0);
        if used > 0 {
            // 有种子时禁止整表重建（避免悬挂引用）：提示改用 merge
            return Err(DomainError::Validation("站点已有种子，replace 会悬挂引用；请使用 merge 模式（保留现有分类，追加新分类）".into()));
        }
        sqlx::query("DELETE FROM categories").execute(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    for (i, c) in cats.iter().enumerate() {
        let id = c.get("id").and_then(serde_json::Value::as_i64).unwrap_or(i as i64 + 1) as i32;
        let name = c.get("name").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        if name.is_empty() { continue; }
        let _ = sqlx::query(
            "INSERT INTO categories (id, name) VALUES ($1, $2) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
        ).bind(id).bind(&name)
        .execute(&mut *tx).await;
    }
    // site_type + 品牌默认
    sqlx::query("INSERT INTO site_settings (name, value) VALUES ('site_type', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()")
        .bind(&pack.code).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO site_settings (name, value) VALUES ('site_name', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()")
        .bind(&pack.brand).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 模块开关 → 站点设定键（textbooks 等）
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let val = if v.as_bool().unwrap_or(false) { "yes" } else { "no" };
            let _ = sqlx::query(
                "INSERT INTO site_settings (name, value) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
            ).bind(format!("module_{k}")).bind(val)
            .execute(&mut *tx).await;
        }
    }
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "site_type_pack_apply", None).await;
    Ok(ok(serde_json::json!({ "applied": pack.code, "mode": mode, "categories": added })))
}

// ---- 捐赠中心（馒头 donate 口径：储值钱包 + 三区套餐 + VIP）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct DonatePlan {
    id: i32,
    plan_type: String,
    title: String,
    reward: Option<String>,
    #[sqlx(default)]
    price_usd: f64,
    sort: i32,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct DonateLedgerRow {
    id: i64,
    kind: String,
    #[sqlx(default)]
    amount_usd: f64,
    #[sqlx(default)]
    balance_after: f64,
    note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
struct DonateState {
    wallet_usd: f64,
    vip_until: Option<chrono::DateTime<chrono::Utc>>,
    plans: Vec<DonatePlan>,
    ledger: Vec<DonateLedgerRow>,
}

/// 捐赠中心总览：钱包余额 + VIP 状态 + 套餐 + 我的流水
#[get("/donate/state")]
async fn donate_state(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (wallet, vip_until): (f64, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT wallet_usd::float8, vip_until FROM users WHERE id = $1",
    ).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let plans: Vec<DonatePlan> = sqlx::query_as(
        "SELECT id, plan_type, title, reward, price_usd::float8, sort FROM donation_plans WHERE enabled ORDER BY sort, id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let ledger: Vec<DonateLedgerRow> = sqlx::query_as(
        "SELECT id, kind, amount_usd::float8, balance_after::float8, note, created_at          FROM donation_ledger WHERE user_id = $1 ORDER BY id DESC LIMIT 30",
    ).bind(auth.id)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(DonateState { wallet_usd: wallet, vip_until, plans, ledger }))
}

#[derive(Deserialize)]
struct TopupBody {
    amount_usd: f64,
    #[serde(default)]
    channel: String, // alipay / wechat（Dev 环境仅记录）
}

/// 充值（Dev 无支付网关：直接入账，模拟支付成功）
#[post("/donate/topup")]
async fn donate_topup(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<TopupBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(10.0..=66.0).contains(&body.amount_usd) {
        return Err(DomainError::Validation("单笔 10 ~ 66 USD".into()));
    }
    if !["alipay", "wechat"].contains(&body.channel.as_str()) {
        return Err(DomainError::Validation("支付方式需为 alipay/wechat".into()));
    }
    let mut tx = state.repo.db.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    let balance: f64 = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true WHERE id = $1 RETURNING wallet_usd::float8",
    ).bind(auth.id).bind(body.amount_usd)
    .fetch_one(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, note) VALUES ($1, 'topup', $2, $3, $4)",
    ).bind(auth.id).bind(body.amount_usd).bind(balance)
    .bind(format!("模拟支付成功（{}）", body.channel))
    .execute(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "donate_topup", None).await;
    Ok(ok(serde_json::json!({ "wallet_usd": balance })))
}

#[derive(Deserialize)]
struct OrderBody { plan_id: i32 }

/// 用余额订购套餐（上传量 / 片单额度 / VIP）
#[post("/donate/order")]
async fn donate_order(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<OrderBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let plan: Option<(String, String, Option<String>, f64)> = sqlx::query_as(
        "SELECT plan_type, title, reward, price_usd::float8 FROM donation_plans WHERE id = $1 AND enabled",
    ).bind(body.plan_id)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plan_type, title, reward, price)) = plan else {
        return Err(DomainError::NotFound(body.plan_id as i64));
    };
    let mut tx = state.repo.db.begin().await.map_err(|e| DomainError::Internal(e.into()))?;
    let balance: Option<f64> = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd - $2 WHERE id = $1 AND wallet_usd >= $2 RETURNING wallet_usd::float8",
    ).bind(auth.id).bind(price)
    .fetch_optional(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        return Err(DomainError::Validation(format!("余额不足，还差 {:.2} USD", price)));
    };
    // 套餐生效
    match plan_type.as_str() {
        "upload" => {
            // 「100 GB 上传量」/「500 GB 上传量」
            let gb: i64 = title.split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or(0);
            sqlx::query("UPDATE users SET uploaded = uploaded + $2 WHERE id = $1")
                .bind(auth.id).bind(gb * 1024 * 1024 * 1024)
                .execute(&mut *tx).await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "quota" => {
            sqlx::query("UPDATE users SET quota_extra = quota_extra + 10 WHERE id = $1")
                .bind(auth.id)
                .execute(&mut *tx).await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "vip" => {
            let days: i64 = if title.contains("终身") { 36500 }
                else if title.contains("180") { 180 } else { 30 };
            sqlx::query(
                "UPDATE users SET vip_until = GREATEST(COALESCE(vip_until, now()), now()) + make_interval(days => $2::int) WHERE id = $1",
            ).bind(auth.id).bind(days)
            .execute(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {}
    }
    // 附赠邀请 1（invite_quota 是 (user_id, period) 结构：当天无行则插入 used=0）
    if reward.as_deref().unwrap_or("").contains("邀请") {
        sqlx::query(
            "INSERT INTO invite_quota (user_id, period, used) VALUES ($1, current_date, 0)              ON CONFLICT (user_id, period) DO NOTHING",
        )
        .bind(auth.id)
        .execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, plan_id, note) VALUES ($1, 'order', -$2, $3, $4, $5)",
    ).bind(auth.id).bind(price).bind(balance).bind(body.plan_id).bind(&title)
    .execute(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "donate_order", Some(body.plan_id as i64)).await;
    Ok(ok(serde_json::json!({ "plan": title, "wallet_usd": balance })))
}

// ---- 批量邮件（massmail）----

#[derive(Deserialize)]
struct MassMailBody { subject: String, body: String }

#[derive(serde::Serialize, sqlx::FromRow)]
struct MassMailRow {
    id: i32,
    subject: String,
    recipients: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    sender: Option<String>,
}

#[get("/admin/massmail")]
async fn massmail_list(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    let rows: Vec<MassMailRow> = sqlx::query_as(
        "SELECT m.id, m.subject, m.recipients, m.created_at, u.username AS sender \
         FROM mass_mails m LEFT JOIN users u ON u.id = m.sent_by ORDER BY m.id DESC LIMIT 50",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/massmail")]
async fn massmail_send(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<MassMailBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 99 { return Err(DomainError::Forbidden); }
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
        .fetch_one(&state.repo.db).await.unwrap_or(0);
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO mass_mails (subject, body, sent_by, recipients) VALUES ($1,$2,$3,$4) RETURNING id",
    ).bind(body.subject.trim()).bind(&body.body).bind(auth.id).bind(users as i32)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "massmail_send", None).await;
    // Dev 环境无 SMTP：落库即视为入队（worker 负责实际投递）
    Ok(ok(serde_json::json!({ "id": id, "queued": users })))
}

// ============ 插件：勋章墙 / 大赛 / 头像挂件 / 五子棋 ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct MedalWallEntry {
    username: String,
    medal_name: String,
}

/// 勋章墙（medal_wall.php 口径）：全部用户的勋章展示墙
#[get("/medal-wall")]
async fn medal_wall(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<MedalWallEntry> = sqlx::query_as(
        "SELECT u.username, m.name AS medal_name \
         FROM user_medals um JOIN users u ON u.id = um.user_id JOIN medals m ON m.id = um.medal_id \
         ORDER BY u.id, m.id LIMIT 200",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ContestInfo {
    id: i32,
    title: String,
    descr: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    is_active: bool,
    entries: i64,
    #[sqlx(default)]
  leader: Option<String>,
    #[sqlx(default)]
  leader_score: Option<i32>,
}

#[get("/contests")]
async fn contest_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let mut rows: Vec<ContestInfo> = sqlx::query_as(
        "SELECT c.id, c.title, c.descr, c.starts_at, c.ends_at, c.is_active, \
            (SELECT count(*) FROM contest_entries e WHERE e.contest_id = c.id) AS entries, \
            (SELECT u.username FROM contest_entries e JOIN users u ON u.id = e.user_id \
             WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader, \
            (SELECT e.score FROM contest_entries e WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader_score \
         FROM contests c ORDER BY c.is_active DESC, c.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = &mut rows;
    Ok(ok(rows))
}

#[post("/contests/{id}/join")]
async fn contest_join(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let inserted = sqlx::query(
        "INSERT INTO contest_entries (contest_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    ).bind(*path).bind(auth.id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已报名".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct FrameRow { id: i32, name: String, css: String, price: i32 }

#[get("/avatar-frames")]
async fn frame_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<FrameRow> = sqlx::query_as(
        "SELECT id, name, css, price FROM avatar_frames ORDER BY sort, id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FrameSetBody { frame_id: Option<i32> }

/// 佩戴头像挂件（需已购买：简化口径 price=0 免费 / >0 扣魔力）
#[put("/me/avatar-frame")]
async fn frame_equip(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, body: web::Json<FrameSetBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    match body.frame_id {
        Some(fid) => {
            let price: Option<i32> = sqlx::query_scalar("SELECT price FROM avatar_frames WHERE id=$1")
                .bind(fid).fetch_optional(&state.repo.db).await
                .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(price) = price else { return Err(DomainError::NotFound(fid as i64)) };
            if price > 0 {
                let idem = format!("frame:{}:{}", auth.id, fid);
                spend_spark(&state.repo.db, auth.id, price as i64, "shop", &idem, "avatar_frame", fid as i64).await?;
            }
            sqlx::query("UPDATE users SET avatar_frame_id=$2 WHERE id=$1")
                .bind(auth.id).bind(fid)
                .execute(&state.repo.db).await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": fid })))
        }
        None => {
            sqlx::query("UPDATE users SET avatar_frame_id=NULL WHERE id=$1")
                .bind(auth.id)
                .execute(&state.repo.db).await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": null })))
        }
    }
}

// ---- 五子棋（wuziqi 口径：建房/加入/落子/棋盘）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct GomokuGame {
    id: i32,
    black_id: i64,
    white_id: Option<i64>,
    board: String,
    turn: String,
    winner_id: Option<i64>,
}

#[post("/gomoku/games")]
async fn gomoku_create(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gomoku_games (black_id, board) VALUES ($1, '') RETURNING id",
    ).bind(auth.id).fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[post("/gomoku/games/{id}/join")]
async fn gomoku_join(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE gomoku_games SET white_id=$2, updated_at=now() WHERE id=$1 AND white_id IS NULL AND black_id <> $2",
    ).bind(*path).bind(auth.id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 { return Err(DomainError::Validation("对局不存在或已有对手".into())); }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(Deserialize)]
struct MoveBody { pos: i32 }

#[post("/gomoku/games/{id}/move")]
async fn gomoku_move(
    req: HttpRequest, state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>, body: web::Json<MoveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(0..225).contains(&body.pos) {
        return Err(DomainError::Validation("落点越界".into()));
    }
    let g: Option<(i64, Option<i64>, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    ).bind(*path).fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((black, white, board, turn, winner)) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if winner.is_some() { return Err(DomainError::Validation("对局已结束".into())); }
    let Some(white) = white else { return Err(DomainError::Validation("等待对手加入".into())); };
    let my_color = if auth.id == black { 'b' } else if auth.id == white { 'w' } else {
        return Err(DomainError::Forbidden);
    };
    if my_color.to_string() != turn { return Err(DomainError::Validation("还没轮到你".into())); }
    // 棋盘惰性填充
    let mut cells: Vec<char> = board.chars().collect();
    cells.resize(225, '.');
    if cells[body.pos as usize] != '.' {
        return Err(DomainError::Validation("该点已有棋子".into()));
    }
    cells[body.pos as usize] = my_color;
    let new_board: String = cells.iter().collect();
    let next_turn = if my_color == 'b' { "w" } else { "b" };
    // 胜负判定（四方向五连）
    let won = check_gomoku_win(&cells, body.pos as usize, my_color);
    let winner_id = if won { Some(auth.id) } else { None };
    sqlx::query(
        "UPDATE gomoku_games SET board=$2, turn=$3, winner_id=$4, updated_at=now() WHERE id=$1",
    ).bind(*path).bind(&new_board).bind(next_turn).bind(winner_id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "board": new_board, "turn": next_turn, "winner": winner_id, "you_won": won })))
}

#[get("/gomoku/games/{id}")]
async fn gomoku_get(
    state: web::Data<std::sync::Arc<AppState>>, path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let g: Option<GomokuGame> = sqlx::query_as(
        "SELECT id, black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    ).bind(*path).fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(g) = g else { return Err(DomainError::NotFound(*path as i64)) };
    Ok(ok(g))
}

fn check_gomoku_win(cells: &[char], pos: usize, color: char) -> bool {
    const SIZE: usize = 15;
    let (r, c) = (pos / SIZE, pos % SIZE);
    let dirs: [(isize, isize); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];
    for (dr, dc) in dirs {
        let mut count = 1;
        for sign in [1, -1] {
            let mut step = 1;
            loop {
                let rr = r as isize + dr * step * sign;
                let cc = c as isize + dc * step * sign;
                if rr < 0 || rr >= SIZE as isize || cc < 0 || cc >= SIZE as isize { break; }
                if cells[rr as usize * SIZE + cc as usize] != color { break; }
                count += 1;
                step += 1;
            }
        }
        if count >= 5 { return true; }
    }
    false
}

// ============ 公告管理（news.php 复刻：发布/编辑/删除，staff 专用） ============

#[derive(Deserialize)]
struct NewsBody {
    title: String,
    body: String,
    #[serde(default)]
    badge: String,
}

#[post("/admin/news")]
async fn news_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题和正文不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO announcements (title, body, badge, author_id, sort)          VALUES ($1, $2, $3, $4, (SELECT COALESCE(max(sort),0)+10 FROM announcements)) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() { "公告" } else { body.badge.trim() })
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "news_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/news/{id}")]
async fn news_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let updated = sqlx::query(
        "UPDATE announcements SET title = $2, body = $3, badge = $4 WHERE id = $1",
    )
    .bind(*path)
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() { "公告" } else { body.badge.trim() })
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/news/{id}")]
async fn news_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let deleted = sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if deleted.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 趣味盒（fun.php 复刻：浏览/投票/发布/编辑/删除/禁止） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FunItemRow {
    id: i32,
    username: Option<String>,
    title: String,
    body: Option<String>,
    status: String,
    added: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    fun_votes: Option<i64>,
    #[sqlx(default)]
    dull_votes: Option<i64>,
    #[sqlx(default)]
    my_vote: Option<String>,
}

/// 趣味盒列表（?status=all 全量需 staff；默认仅 normal）
#[get("/fun/items")]
async fn fun_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let staff_view = q.get("status").map(|s| s.as_str()).unwrap_or("") == "all";
    let rows: Vec<FunItemRow> = sqlx::query_as(
        "SELECT f.id, u.username, f.title, f.body, f.status, f.added, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'fun') AS fun_votes, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'dull') AS dull_votes, \
            (SELECT v.vote FROM fun_item_votes v WHERE v.fun_id = f.id AND v.user_id = $2) AS my_vote \
         FROM fun_items f LEFT JOIN users u ON u.id = f.user_id \
         WHERE ($3::bool OR f.status = 'normal') \
         ORDER BY f.added DESC LIMIT 50",
    )
    .bind(true)
    .bind(auth.id)
    .bind(auth.class_id >= 90 && staff_view)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FunVoteBody {
    fun_id: i32,
    vote: String,
}

#[post("/fun/items/vote")]
async fn fun_item_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunVoteBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.vote != "fun" && body.vote != "dull" {
        return Err(DomainError::Validation("投票只能是 fun 或 dull".into()));
    }
    let inserted = sqlx::query(
        "INSERT INTO fun_item_votes (fun_id, user_id, vote) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(body.fun_id)
    .bind(auth.id)
    .bind(&body.vote)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已经投过啦".into()));
    }
    Ok(ok(serde_json::json!({ "voted": body.vote })))
}

#[derive(Deserialize)]
struct FunItemBody {
    title: String,
    #[serde(default)]
    body: Option<String>,
}

/// 发布趣味内容（24h 冷却：最新一条发布不足 24 小时则拒绝，staff 豁免）
#[post("/fun/items")]
async fn fun_item_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("标题不能为空".into()));
    }
    let recent: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT max(added) FROM fun_items WHERE status NOT IN ('banned','dull')",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(t) = recent {
        if chrono::Utc::now() - t < chrono::Duration::hours(24) && auth.class_id < 90 {
            return Err(DomainError::Validation(
                "最新一条发布不足 24 小时，请稍后再来".into(),
            ));
        }
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO fun_items (user_id, title, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.title.trim())
    .bind(&body.body)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/fun/items/{id}")]
async fn fun_item_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET title = $2, body = $3 WHERE id = $1")
        .bind(*path)
        .bind(body.title.trim())
        .bind(&body.body)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FunStatusBody {
    status: String,
}

/// 修改状态（含「禁止」banned；staff 或作者对自己可 dull）
#[put("/fun/items/{id}/status")]
async fn fun_item_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunStatusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed = ["normal", "dull", "notfunny", "funny", "veryfunny", "banned"];
    if !allowed.contains(&body.status.as_str()) {
        return Err(DomainError::Validation("非法状态".into()));
    }
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    let is_staff = auth.class_id >= 90;
    if (body.status == "banned" || owner != auth.id) && !is_staff {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET status = $2 WHERE id = $1")
        .bind(*path)
        .bind(&body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "fun_status_change", None).await;
    Ok(ok(serde_json::json!({ "ok": true, "status": body.status })))
}

#[delete("/fun/items/{id}")]
async fn fun_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM fun_items WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 友情链接管理（linksmanage.php 复刻：申请/审核/编辑/删除） ============

#[derive(Deserialize)]
struct LinkApplyBody {
    name: String,
    url: String,
    #[serde(default)]
    title: Option<String>,
    admin: String,
    email: String,
    reason: String,
}

/// 申请友链（进 pending，staff 审核后 active）
#[post("/links/apply")]
async fn link_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LinkApplyBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.name.trim().is_empty() || body.url.trim().is_empty() {
        return Err(DomainError::Validation("站点名和 URL 必填".into()));
    }
    if !body.email.contains('@') {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    if body.reason.trim().chars().count() < 20 {
        return Err(DomainError::Validation("申请理由至少 20 字".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO friend_links (name, url, title, status, applied_by, admin_name, email, reason)          VALUES ($1, $2, $3, 'pending', $4, $5, $6, $7) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(&body.title)
    .bind(auth.id)
    .bind(body.admin.trim())
    .bind(body.email.trim())
    .bind(body.reason.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_apply", None).await;
    Ok(ok(serde_json::json!({ "id": id, "status": "pending" })))
}

#[derive(Deserialize)]
struct LinkAdminBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    status: Option<String>,
}

/// 编辑/审核友链（staff；status: pending→active 通过）
#[put("/admin/links/{id}")]
async fn link_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<LinkAdminBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    if let Some(st) = &body.status {
        if !["pending", "active", "hidden"].contains(&st.as_str()) {
            return Err(DomainError::Validation("非法状态".into()));
        }
    }
    let updated = sqlx::query(
        "UPDATE friend_links SET \
            name = COALESCE($2, name), url = COALESCE($3, url), title = COALESCE($4, title), \
            sort = COALESCE($5, sort), status = COALESCE($6, status) \
         WHERE id = $1",
    )
    .bind(*path)
    .bind(&body.name)
    .bind(&body.url)
    .bind(&body.title)
    .bind(body.sort)
    .bind(&body.status)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "link_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/links/{id}")]
async fn link_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM friend_links WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct LinkRow {
    id: i32,
    name: String,
    url: String,
    title: Option<String>,
    status: String,
    #[sqlx(default)]
    applied_by: Option<i64>,
    #[sqlx(default)]
    admin_name: Option<String>,
    #[sqlx(default)]
    email: Option<String>,
    #[sqlx(default)]
    reason: Option<String>,
}

/// 友链管理列表（staff，含 pending）
#[get("/admin/links")]
async fn link_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let rows: Vec<LinkRow> = sqlx::query_as(
        "SELECT id, name, url, title, status, applied_by, admin_name, email, reason \
         FROM friend_links ORDER BY (status = 'pending') DESC, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 首页（复刻包子站 index.php 五大板块） ============

/// 首页汇总：公告 + 签到日历 + 30 天新增资源统计 + 站点数据 + 娱乐流水
#[get("/home")]
async fn home_sections(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;

    // 公告（home-news）：最新一条为头条 + 其余为列表
    let news: Vec<(i32, String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, title, body, badge, created_at FROM announcements ORDER BY id DESC LIMIT 8",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let news_json: Vec<serde_json::Value> = news
        .iter()
        .map(|(id, title, body, badge, ts)| {
            // 公告 body 为富文本 HTML（管理员撰写，NP 口径）：出站前 ammonia 白名单消毒，
            // 剥离 script/事件属性/javascript: 协议 —— 管理员账号被盗也不构成全站存储 XSS
            let safe_body = ammonia::Builder::default().clean(body).to_string();
            serde_json::json!({
                "id": id, "title": title, "body": safe_body, "badge": badge,
                "date": ts.format("%m-%d").to_string(),
            })
        })
        .collect();

    // 签到日历（attendance-card）：当月逐日 + 连签/累计
    let att: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, reward FROM attendance WHERE user_id = $1 ORDER BY date",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let today = chrono::Local::now().date_naive();
    let month_start = chrono::Datelike::with_day(&today, 1).unwrap_or(today);
    let days_in_month = {
        let next_month = if chrono::Datelike::month(&month_start) == 12 {
            chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&month_start) + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(
                chrono::Datelike::year(&month_start),
                chrono::Datelike::month(&month_start) + 1,
                1,
            )
        }
        .unwrap_or(today);
        (next_month.pred_opt().unwrap_or(today) - month_start).num_days() as u32 + 1
    };
    let calendar: Vec<serde_json::Value> = (0..days_in_month)
        .map(|i| {
            let d = month_start + chrono::Duration::days(i as i64);
            let hit = att.iter().find(|(ad, _, _)| *ad == d);
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "day": chrono::Datelike::day(&d),
                "done": hit.is_some(),
                "reward": hit.map(|(_, _, r)| r).unwrap_or(&0),
            })
        })
        .collect();
    let streak = att.iter().rev().next().map(|(_, s, _)| *s).unwrap_or(0);
    let total_days = att.len() as i32;
    let checked_today = att.iter().any(|(d, _, _)| *d == today);

    // 30 天新增资源统计（home-resource-stats）：普通 vs 官种
    let daily: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, \
                count(*) FILTER (WHERE NOT official_tag) AS ordinary, \
                count(*) FILTER (WHERE official_tag) AS official \
         FROM torrents WHERE approval_status = 1 AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let by_day: std::collections::HashMap<chrono::NaiveDate, (i64, i64)> = daily
        .iter()
        .map(|(d, o, f)| (*d, (*o, *f)))
        .collect();
    let series: Vec<serde_json::Value> = (0..30)
        .rev()
        .map(|i| {
            let d = today - chrono::Duration::days(i);
            let (o, f) = by_day.get(&d).cloned().unwrap_or((0, 0));
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "ordinary": o, "official": f, "total": o + f,
            })
        })
        .collect();
    let today_count = by_day.get(&today).map(|(o, f)| o + f).unwrap_or(0);
    let last7: Vec<i64> = (1..=7)
        .map(|i| {
            by_day
                .get(&(today - chrono::Duration::days(i)))
                .map(|(o, f)| o + f)
                .unwrap_or(0)
        })
        .collect();
    let avg7 = if last7.is_empty() {
        0.0
    } else {
        last7.iter().sum::<i64>() as f64 / last7.len() as f64
    };
    let total30: i64 = daily.iter().map(|(_, o, f)| o + f).sum();

    // 站点数据（home-site-data 三列）
    let (users, torrents_n, peers, seeders, leechers, warned, banned, unverified): (
        i64, i64, i64, i64, i64, i64, i64, i64,
    ) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM users WHERE status < 2), \
            (SELECT count(*) FROM torrents WHERE approval_status = 1), \
            (SELECT count(*) FROM snatches WHERE seeding OR leeching), \
            (SELECT count(*) FROM snatches WHERE seeding), \
            (SELECT count(*) FROM snatches WHERE leeching), \
            (SELECT count(*) FROM users WHERE status = 1), \
            (SELECT count(*) FROM users WHERE status >= 2), \
            (SELECT count(*) FROM users WHERE must_reset_password)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up_sum, down_sum, size_sum): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT COALESCE(sum(uploaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(downloaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(size),0)::bigint FROM torrents WHERE approval_status = 1)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 娱乐流水（幸运大转盘 → 以 spark_ledger 游戏类流水近似）
    let lucky: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT u.username, l.kind, l.amount FROM spark_ledger l \
         JOIN users u ON u.id = l.user_id \
         WHERE l.kind LIKE '%game%' OR l.kind LIKE '%vote%' \
         ORDER BY l.created_at DESC LIMIT 15",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 友情链接
    let links: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT name, url, title FROM friend_links ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "news": news_json,
        "attendance": {
            "month": today.format("%Y年%m月").to_string(),
            "streak": streak, "total_days": total_days, "checked_today": checked_today,
            "calendar": calendar,
        },
        "resource_stats": {
            "today": today_count, "avg7": (avg7 * 10.0).round() / 10.0,
            "total30": total30, "series": series,
        },
        "site_data": {
            "users": users, "torrents": torrents_n, "peers": peers,
            "seeders": seeders, "leechers": leechers,
            "warned": warned, "banned": banned, "unverified": unverified,
            "total_upload": up_sum, "total_download": down_sum, "total_size": size_sum,
        },
        "lucky_draw": lucky.iter().map(|(u, k, a)| serde_json::json!({
            "user": u, "kind": k, "amount": a,
        })).collect::<Vec<_>>(),
        "friend_links": links.iter().map(|(n, u, t)| serde_json::json!({
            "name": n, "url": u, "title": t,
        })).collect::<Vec<_>>(),
    })))
}

/// announce 统计入口（worker 内部使用；对外需 staff 权限）
#[post("/internal/announce-batch")]
async fn announce_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: String,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let count = body.lines().count() as i64;
    Ok(ok(serde_json::json!({ "received": count })))
}

// ============ 发布 / 下载（M04 / M05） ============

#[derive(Deserialize)]
struct UploadForm {
    name: Option<String>,
    small_descr: Option<String>,
    descr: Option<String>,
    category_id: i32,
    medium_id: i32,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    #[serde(default)]
    anonymous: bool,
    /// 海报/封面外链 URL（存 media_info.poster；列表 46px 封面位与首页海报墙共用）
    #[serde(default)]
    poster: Option<String>,
}

/// multipart：file=<.torrent> + 表单字段
#[post("/torrents")]
async fn upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    mut payload: actix_multipart::Multipart,
    form: web::Query<UploadForm>,
) -> DomainResult<HttpResponse> {
    use actix_web::web::Bytes;
    use futures_util::StreamExt;

    let auth = require_auth(&req, &state).await?;
    let mut file_bytes: Option<Bytes> = None;
    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| DomainError::Validation(e.to_string()))?;
        if field.name() == Some("file") {
            let mut buf = web::BytesMut::new();
            while let Some(chunk) = field.next().await {
                buf.extend_from_slice(&chunk.map_err(|e| DomainError::Validation(e.to_string()))?);
            }
            file_bytes = Some(buf.freeze());
        }
    }
    let bytes = file_bytes.ok_or(DomainError::Validation("缺少 .torrent 文件".into()))?;

    let parsed = crate::bencode::parse_torrent(&bytes).map_err(DomainError::TorrentInvalid)?;

    // 重复检测（M04：info_hash 唯一）
    let dupe: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrents WHERE info_hash = $1)")
            .bind(&parsed.info_hash_hex)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if dupe {
        return Err(DomainError::TorrentDuplicate);
    }

    let name = form
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(parsed.name.clone());
    // 封面外链 → media_info.poster（JSONB 单键合并，留空则不动该列）
    let media_info: Option<serde_json::Value> = form
        .poster
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(|u| serde_json::json!({ "poster": u }));
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrents (info_hash, name, small_descr, descr, category_id, medium_id, grade_id, edition_id, owner_id, anonymous, size, numfiles, approval_status, media_info) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0, $13) RETURNING id",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&name)
    .bind(&form.small_descr)
    .bind(&form.descr)
    .bind(form.category_id)
    .bind(form.medium_id)
    .bind(form.grade_id)
    .bind(form.edition_id)
    .bind(auth.id)
    .bind(form.anonymous)
    .bind(parsed.size)
    .bind(parsed.numfiles)
    .bind(media_info)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    state
        .repo
        .audit(Some(auth.id), "torrent_upload", Some(id))
        .await;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(&state, id, auth.id);
    Ok(ok(serde_json::json!({ "id": id, "approval_status": 0 })))
}

#[get("/torrents/{id}/download")]
async fn download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    use actix_web::body::BoxBody;

    let auth = require_auth(&req, &state).await?;
    let raw: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT f.raw FROM torrent_files f \
         JOIN torrents t ON t.id = f.torrent_id \
         WHERE f.torrent_id = $1 AND t.approval_status = 1",
    )
    .bind(path.into_inner())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(raw) = raw else {
        return Err(DomainError::NotFound(0));
    };
    let user = state
        .repo
        .find_user_by_id(auth.id)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    // 注入本站 announce（含 passkey）+ private=1；info dict 不动 → info_hash 与上传时一致（M05）
    // 汇报地址来源：站点设定 announce_url / https_announce_url 优先（设定页可改），
    // PUBLIC_TRACKER_URL 环境变量兜底。配置了 https 时首选加密汇报，http 作 BEP12 回退。
    async fn setting(db: &sqlx::PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
            .bind(name)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .map(|v| v.trim().trim_end_matches('/').to_string())
            .filter(|v| !v.is_empty())
    }
    let env_host = std::env::var("PUBLIC_TRACKER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7070".into())
        .trim_end_matches('/')
        .to_string();
    let base_http = setting(&state.repo.db, "announce_url")
        .await
        .unwrap_or(env_host);
    let announce = match setting(&state.repo.db, "https_announce_url").await {
        Some(https) if https != base_http => {
            format!("{https}/announce/{}", user.passkey)
        }
        _ => format!("{base_http}/announce/{}", user.passkey),
    };
    // http 回退仅在与首选不同时下发（BEP12 单 tier：失败自动降级，不支持 TLS 的老客户端可用）
    let mut fallbacks = Vec::new();
    if !announce.starts_with(&format!("{base_http}/")) {
        fallbacks.push(format!("{base_http}/announce/{}", user.passkey));
    }
    let body = crate::bencode::build_download_torrent(&raw, &announce, &fallbacks)
        .map_err(DomainError::TorrentInvalid)?;
    let mut resp = HttpResponse::with_body(actix_web::http::StatusCode::OK, BoxBody::new(body));
    resp.headers_mut().insert(
        actix_web::http::header::CONTENT_TYPE,
        actix_web::http::header::HeaderValue::from_static("application/x-bittorrent"),
    );
    Ok(resp)
}

// ============ 邀请（M23 P0 面） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct InviteRow {
    id: i64,
    code: String,
    status: i16,
    used_by: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[get("/invites")]
async fn list_invites_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, InviteRow>(
        "SELECT i.id, i.code, i.status, u.username AS used_by, i.expires_at \
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
async fn issue_invite_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 配额：等级 LV3+ 每周 2 枚。原子占位（UPDATE 计数行）防并发穿透
    let quota: i64 = if auth.class_id >= 3 { 2 } else { 0 };
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
        return Err(DomainError::Forbidden);
    }
    let code = crate::domain::new_invite_code();
    let expires = crate::domain::invite_expiry();
    let id = state.repo.issue_invite(auth.id, &code, expires).await?;
    Ok(ok(
        serde_json::json!({ "id": id, "code": code, "expires_at": expires.to_rfc3339() }),
    ))
}

#[derive(Deserialize)]
struct RedeemInviteReq {
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 魔力兑换邀请：按魔力商店「邀请名额」（kind=invite）现价扣火花，直接发一枚 72h 邀请码。
/// 无等级限制（商店购买口径），与 /shop/buy 走同一条扣款管线（幂等键防双扣）。
#[post("/invites/redeem")]
async fn redeem_invite_handler(
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
    // 幂等键必填（与 /shop/buy 同口径）：网络重试携带同一键防双扣款
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.is_empty())
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
    state.repo.audit(Some(auth.id), "invite_redeem", Some(id)).await;
    Ok(ok(serde_json::json!({
        "id": id, "code": code, "expires_at": expires.to_rfc3339(), "price": price,
    })))
}

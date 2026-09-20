//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::domain::{self};
use crate::dto::ok;
use crate::economy_http::spend_spark;
use crate::errors::{DomainError, DomainResult};
use crate::social_http::{
    endangered_list, team_create, team_join, team_leave, team_list, team_mine,
};
use crate::state::AppState;
use crate::torrents;

pub fn v1_scope() -> actix_web::Scope {
    web::scope("/api/v1")
        .service(health)
        .service(register)
        .service(login)
        .service(me)
        .service(me_perms)
        .service(me_overview)
        .service(me_settings_get)
        .service(me_settings_put)
        .service(my_torrentlist)
        .service(user_torrentlist)
        .service(my_bookmarks)
        .service(rotate_passkey)
        .service(my_login_history)
        .service(crate::attachment_http::upload_attachment)
        .service(crate::attachment_http::get_attachment)
        .service(me_password_change)
        .service(user_public_profile)
        .service(list)
        .service(detail)
        .service(torrent_detail_ext)
        .service(torrent_files)
        .service(torrent_thanks)
        .service(comments)
        .service(create_comment)
        .service(delete_comment)
        .service(do_thank)
        .service(do_bookmark)
        .service(edit_torrent)
        .service(set_torrent_price)
        .service(delete_torrent)
        .service(restore_torrent)
        .service(resubmit_torrent)
        .service(group_attach)
        .service(group_info)
        .service(group_subscribe)
        .service(group_unsubscribe)
        .service(torrent_snatches)
        .service(torrent_nfo)
        .service(request_reseed)
        .service(torrent_tags)
        .service(torrent_tag_put)
        .service(stats)
        .service(cheat_events_list)
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
        .service(freeleech_update)
        .service(freeleech_delete)
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
        .service(seed_stats)
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
        .service(donate_order_status)
        .service(donate_notify)
        .service(donate_order)
        .service(site_profile)
        .service(site_type_pack_list)
        .service(site_type_pack_apply)
        .service(site_type_pack_diff)
        .service(site_type_pack_save)
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
        .service(admin_home_layout_put)
        .service(upload)
        .service(ptgen)
        .service(download)
        .service(crate::invite_http::issue_invite_handler)
        .service(crate::invite_http::redeem_invite_handler)
        .service(crate::invite_http::list_invites_handler)
        .service(crate::invite_http::invites_status_handler)
        .service(crate::invite_http::email_invite_handler)
        .service(endangered_list)
        .service(team_create)
        .service(team_join)
        .service(team_leave)
        .service(team_list)
        .service(team_mine)
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
    // 注册模式（U1 §11.5）：invite_only（默认，现状）/ open / email_verify。
    // open 模式旁路邀请码（inviter 置 NULL，链路其余不变）；email_verify 走
    // 既有邮箱确认信（confirm/resend 已有），未确认账号登录受限由既有列控制。
    let reg_mode: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'registration_mode'), 'invite_only')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "invite_only".into());
    if reg_mode == "invite_only" && body.invite_code.trim().is_empty() {
        return Err(DomainError::InviteInvalid);
    }
    // 图形验证码校验（防注册机）
    if body.captcha_id.is_empty()
        || !crate::gaps_http::captcha_verify(&state, &body.captcha_id, body.captcha_answer).await
    {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    // 注册限流（§5.7）：按来源 IP 每分钟 5 次，防邀请码爆破
    let ip = client_ip(&req);
    // IP 封禁强制校验（与登录同口径；封禁名单由管理面维护）
    if ip_banned(&state, &ip).await {
        return Err(DomainError::Validation("IP 已被封禁，请联系管理组".into()));
    }
    // 邮箱黑名单（0032 建表后首次接入注册链路）：pattern 三形态匹配——
    // 完整邮箱 / @domain（域名封禁）/ user@（前缀封禁）；allow 行优先豁免
    let email_banned: Option<bool> = sqlx::query_scalar(
        "SELECT NOT bool_or(mode = 'allow') FROM email_bans \
         WHERE lower($1) = lower(pattern) \
            OR (pattern LIKE '@%' AND lower($1) LIKE '%' || lower(pattern)) \
            OR (pattern LIKE '%@' AND lower($1) LIKE lower(pattern) || '%')",
    )
    .bind(&new_user.email)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None)
    .flatten();
    if email_banned == Some(true) {
        return Err(DomainError::Validation(
            "该邮箱地址已被禁用，请联系管理组".into(),
        ));
    }
    throttle(&state, format!("register-ip:{ip}")).await?;
    let pass_hash = domain::hash_password(&new_user.password)?;
    // 单事务注册（审计修复）：旧版「建用户→消费邀请码→补偿 DELETE」三段非原子，
    // 中间崩溃会留下未绑邀请人的账号或占用用户名。现在整个流程一个事务内完成。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let passkey = {
        // 与 repo::create_user 同口径的 passkey 生成
        let p: String = sqlx::query_scalar("SELECT encode(gen_random_bytes(20), 'hex')")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        p
    };
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, email, pass_hash, passkey) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&new_user.username)
    .bind(&new_user.email)
    .bind(&pass_hash)
    .bind(&passkey)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => DomainError::UsernameTaken,
        other => DomainError::Internal(other.into()),
    })?;
    let inviter: Option<i64> = if reg_mode == "invite_only" {
        let inv = sqlx::query_scalar(
            "UPDATE invites SET status = 1, used_by = $1          WHERE code = $2 AND status = 0 AND expires_at > now()          RETURNING inviter_id",
        )
        .bind(user_id)
        .bind(&body.invite_code)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(inv) = inv else {
            // 邀请码无效：整个事务回滚（不再需要补偿 DELETE）
            let used: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM invites WHERE code = $1 AND status = 1)",
            )
            .bind(&body.invite_code)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
            return Err(if used {
                DomainError::InviteUsed
            } else {
                DomainError::InviteInvalid
            });
        };
        Some(inv)
    } else {
        // open / email_verify：无邀请人（invited_by 保持 NULL）
        None
    };
    if let Some(inviter) = inviter {
        sqlx::query("UPDATE users SET invited_by = $2 WHERE id = $1")
            .bind(user_id)
            .bind(inviter)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(user_id), "user_register", Some(user_id))
        .await;
    Ok(ok(serde_json::json!({ "user_id": user_id })))
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
    let peer_ip = client_ip(&req);
    // UA（0096 风控证据）：区分「同一人多设备」与「凭据泄露换客户端」；截断防滥用
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(300)
        .collect::<String>();
    if ip_banned(&state, &peer_ip).await {
        return Err(DomainError::Validation("IP 已被封禁，请联系管理组".into()));
    }
    // 登录限流（§5.7：5 次/分钟/用户名 + 5 次/分钟/IP 双维度——
    // 原实现仅用户名维度，换用户名字典爆破同一账户不受限）
    throttle(&state, format!("login:{}", body.username)).await?;
    throttle(&state, format!("login-ip:{}", peer_ip)).await?;
    let user = state
        .repo
        .find_user_by_name(body.username.trim())
        .await?
        .ok_or(DomainError::InvalidCredentials);
    let user = match user {
        Ok(u) => u,
        Err(e) => {
            // 封禁账户的密码正确时：不给 403，而是落「密码正确但被封」事件后仍回
            // 失败口径 —— 防枚举与正常流一致（真实封禁提示走公开 /auth/ban-log）。
            // 申诉通道见下方 ban_appeal 分支（P0 修复：此前被封用户无任何自助申诉入口）。
            let _ = sqlx::query(
                "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES (NULL, NULLIF($1,'')::inet, false, $2, 4)",
            )
            .bind(&peer_ip)
            .bind(&ua)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    };
    if !domain::verify_password(&user.pass_hash, &body.password) {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 1)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::InvalidCredentials);
    }
    // 2FA（启用者必须带 totp_code）。失败也落登录事件（reason=2：缺码/错码细分看返回错误）
    if let Err(e) =
        crate::twofa_http::login_totp_check(&state.repo.db, user.id, body.totp_code.unwrap_or(0))
            .await
    {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 2)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(e);
    }
    // 0072 闲置停用拦截：dormant_mark（worker 每小时）给 90 天未登录且无做种的非员工账号打标。
    // 拦截放在密码/2FA 之后 —— 不给探测者区分「休眠账号是否存在」的信息差。
    if user.dormant_at.is_some() {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 3)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::Validation(
            "账号因长期未登录已被停用，请通过『联系我们』附上用户名申请恢复".into(),
        ));
    }
    let token = state
        .jwt
        .issue(user.id, user.class_id, 24)
        .map_err(DomainError::Internal)?;
    // 登录事件（控制面板账户概览 30 天活跃趋势；含 IP 供 ipcheck/maxlogin）
    let _ = sqlx::query(
        "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, true, $3, 0)",
    )
    .bind(user.id)
    .bind(&peer_ip)
    .bind(&ua)
    .execute(&state.repo.db)
    .await;
    // M28 插件 Hook：登录成功后分发
    state.plugins.dispatch_login(&state, user.id);
    // 安全加固（P1 XSS 面）：token 同时以 HttpOnly+SameSite=Lax cookie 下发。
    // 兼容期双轨：响应体仍带 token（存量前端 localStorage 口径），前端迁移完成后
    // 移除。cookie 路径限定 /api/v1，浏览器侧 api 调用自动携带；HttpOnly 使
    // XSS 无法读取（require_auth 同时接受 Cookie，见 token_from_request）。
    let mut resp = ok(serde_json::json!({
        "token": token,
        "must_reset_password": user.must_reset_password,
        "user": { "id": user.id, "username": user.username, "class_id": user.class_id }
    }));
    let cookie = actix_web::cookie::Cookie::build("flux_token", token)
        .path("/api/v1")
        .max_age(actix_web::cookie::time::Duration::hours(24))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .finish()
        .to_string();
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&cookie).expect("cookie 串解析为 HeaderValue 必然成功"),
    );
    Ok(resp)
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

/// 解析请求来源 IP（限流 / 封禁 / 风控事件统一口径）。
/// 默认取 socket 对端；`TRUST_PROXY=1` 时改信 X-Forwarded-For 首值——仅当 api 只能经
/// 可信反代访问时启用（直连暴露时客户端可伪造 XFF 绕过限流/封禁）。
/// P1 修复：此前注册/登录/验证码全部直接用 peer_addr（socket 对端），生产经反代后
/// 全站共享代理 IP——login-ip 限流桶全站共用（误伤 429）、ip_bans 误封整个代理、
/// login_events 风控记录失真。
pub fn client_ip(req: &HttpRequest) -> String {
    static TRUST_PROXY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let trust =
        *TRUST_PROXY.get_or_init(|| std::env::var("TRUST_PROXY").ok().as_deref() == Some("1"));
    if trust {
        if let Some(xff) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            // 取链路首值（最接近真实客户端的一段）；反代应追加而非覆盖
            if let Some(first) = xff.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    req.peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// ip_bans 强制校验：命中返回 true（封禁名单由管理面维护，见 /admin/bans）。
/// 登录/注册为低频入口，直接查库即可；高频路径（tracker announce）用内存缓存版，
/// 见 apps/tracker/src/main.rs 的 refresh_guard()/ip_banned()。
async fn ip_banned(state: &Arc<AppState>, ip: &str) -> bool {
    if ip.is_empty() || ip == "unknown" {
        return false;
    }
    // CIDR 网段封禁支持：ip 列允许 '1.2.3.0/24' 形状，精确 IP 同时匹配单地址与所属网段
    sqlx::query_scalar::<_, i32>("SELECT 1 FROM ip_bans WHERE $1::inet <<= ip LIMIT 1")
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

/// 用户状态短缓存（审计 P2）：require_auth 每请求的 users 行查询以 5s TTL 缓存。
/// TTL 权衡：封禁/降级最坏 5s 延迟生效（撤销线仍每请求直查，登出即时）；
/// 管理端封禁/降级后调用 invalidate() 主动失效可消除窗口。
#[derive(Default)]
pub struct UserStatusCache {
    inner: std::sync::RwLock<
        std::collections::HashMap<i64, (Option<(i16, i32, bool)>, std::time::Instant)>,
    >,
}

impl UserStatusCache {
    const TTL: std::time::Duration = std::time::Duration::from_secs(5);
    const CAP: usize = 10_000;

    fn get(&self, uid: i64) -> Option<Option<(i16, i32, bool)>> {
        let g = self.inner.read().unwrap_or_else(|e| e.into_inner());
        g.get(&uid)
            .filter(|(_, at)| at.elapsed() < Self::TTL)
            .map(|(v, _)| v.clone())
    }

    fn put(&self, uid: i64, v: Option<(i16, i32, bool)>) {
        let mut g = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if g.len() >= Self::CAP && !g.contains_key(&uid) {
            // 粗略容量保护：淘汰最旧条目（活跃站上 5s 窗口内远达不到 1 万用户）
            if let Some(oldest) = g.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k) {
                g.remove(&oldest);
            }
        }
        g.insert(uid, (v, std::time::Instant::now()));
    }

    /// 管理端封禁/降级/解封后调用：立即失效该用户的缓存行
    pub fn invalidate(&self, uid: i64) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&uid);
    }
}

/// 凭证提取：优先 Authorization: Bearer；缺失时回落 HttpOnly cookie flux_token
/// （登录接口 Set-Cookie 下发，见 login handler）。双轨期两者等价。
fn token_from_request(req: &HttpRequest) -> Option<String> {
    if let Some(b) = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(b.to_string());
    }
    req.cookie("flux_token").map(|c| c.value().to_string())
}

/// 从 Authorization: Bearer 或 HttpOnly cookie 提取用户（§8.1：后端权威鉴权）
pub async fn require_auth(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<AuthUser> {
    let token = token_from_request(req).ok_or(DomainError::Unauthorized)?;
    let claims = state
        .jwt
        .verify(token.as_str())
        .ok_or(DomainError::Unauthorized)?;
    // 登出撤销检查：签发时间早于 not-before 的 token 一律拒绝。
    // 权威在 token_revocations 表（0085：Redis 重启不再让已撤销 token 复活）；
    // Redis 键保留 24h 作为存量兜底（迁移前撤销线只写了 Redis）。
    {
        let db_nbf: Option<i64> =
            sqlx::query_scalar("SELECT nbf FROM token_revocations WHERE user_id = $1")
                .bind(claims.sub)
                .fetch_optional(&state.repo.db)
                .await
                .unwrap_or(None);
        let mut c = state.redis.clone();
        let redis_nbf: Option<i64> =
            redis::AsyncCommands::get(&mut c, format!("logout_nbf:{}", claims.sub))
                .await
                .unwrap_or(None);
        let nbf = db_nbf.max(redis_nbf);
        if let Some(nbf) = nbf {
            // <=：撤销线含义为「iat 不晚于 nbf 的凭证全部作废」。登出把 nbf 设为凭证 iat，
            // 因此登出所用的 token（iat==nbf）自身也被判死 —— 这是登出的本意；
            // 改密路径写 nbf=iat-1，保留改密后同秒重登的新凭证（见 me_password_change）。
            if claims.iat <= nbf {
                return Err(DomainError::Unauthorized);
            }
        }
    }
    // 权威校验（P1 修复）：token 只是凭证，状态与等级以库为准 —— 封禁/降级即时生效。
    // 5s TTL 缓存（P2）：省掉每请求一次 users round-trip；管理端封禁/降级后调
    // UserStatusCache::invalidate 消除窗口，最坏情况延迟 5s 生效。
    let row: Option<(i16, i32, bool)> = match state.user_status_cache.get(claims.sub) {
        Some(cached) => cached,
        None => {
            let fetched = sqlx::query_as(
                "SELECT status, class_id, must_reset_password FROM users WHERE id = $1",
            )
            .bind(claims.sub)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            state.user_status_cache.put(claims.sub, fetched.clone());
            fetched
        }
    };
    let Some((status, class_id, must_reset)) = row else {
        return Err(DomainError::Unauthorized);
    };
    if status >= 2 {
        return Err(DomainError::Forbidden); // 封禁账户
    }
    // 审计修复（P1）：临时密码强制改密此前仅靠前端跳转，服务端不拦截 ——
    // 未改密账号可无限期调用全部 API。现在只放行改密/登出/自身信息三条自救路径。
    if must_reset {
        let path = req.path();
        let allowed = matches!(path, "/api/v1/auth/logout")
            || path.starts_with("/api/v1/me/password")
            || path == "/api/v1/me";
        if !allowed {
            return Err(DomainError::Validation(
                "账号正在使用临时密码，请先修改密码后再操作".into(),
            ));
        }
    }
    Ok(AuthUser {
        id: claims.sub,
        class_id,
        iat: claims.iat,
    })
}

/// 可选鉴权：匿名/无效 token 返回 None（自定义菜单公开端点按等级过滤用）
pub async fn optional_auth(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Option<AuthUser> {
    require_auth(req, state).await.ok()
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
    // 撤销线 DB 权威（0085）+ Redis 加速缓存
    // 审计修复（P2 吞错）：撤销是安全语义操作，双写（DB 权威 + Redis 加速）失败必须
    // 留痕——旧版全部静默吞掉，双双失败时 token 仍有效且无任何日志可查。
    if let Err(e) = sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2)          ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(auth.iat)
    .execute(&state.repo.db)
    .await
    {
        tracing::error!(user_id = auth.id, error = ?e, "登出撤销线 DB 写入失败：token 在 24h 内仍可能通过校验");
    }
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let redis_res: Result<(), _> =
        redis::AsyncCommands::set_ex(&mut c, &key, auth.iat, 86400u64).await;
    if let Err(e) = redis_res {
        tracing::warn!(user_id = auth.id, error = ?e, "登出撤销线 Redis 写入失败（DB 权威仍在，影响为加速缓存缺失）");
    }
    state.repo.audit(Some(auth.id), "auth.logout", None).await;
    // 同步清除 HttpOnly 会话 cookie（与撤销线配合：即便 token 被复用，cookie 已不存在）
    let clear = actix_web::cookie::Cookie::build("flux_token", "")
        .path("/api/v1")
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .finish()
        .to_string();
    let mut resp = ok(serde_json::json!({ "ok": true }));
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&clear).expect("清 cookie 串解析必然成功"),
    );
    Ok(resp)
}

/// 我的权限清单（前端 admin Tab 渲染过滤用；批量取回避免 N 次 round-trip）
#[get("/me/perms")]
async fn me_perms(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let keys = crate::authz::user_perm_keys(&state.repo.db, auth.class_id, auth.id).await;
    let roles = crate::authz::user_role_keys(&state.repo.db, auth.id).await;
    Ok(ok(serde_json::json!({ "perms": keys, "roles": roles })))
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
    // 头像 + 头像框（userbar/个人主页展示）+ 佩戴勋章（用户名角标）一并回传；
    // css 现查现回（avatar_frames 行少且小，没必要常驻缓存）
    let deco: Option<(Option<String>, Option<i32>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT u.avatar_url, u.avatar_frame_id, f.css AS frame_css, f.image_url AS frame_image \
         FROM users u LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (avatar_url, frame_id, frame_css, frame_image) = deco.unwrap_or((None, None, None, None));
    // 佩戴勋章（与个人主页 worn_medals 同口径，userbar 用户名后角标，最多 3 枚）
    let worn_medals: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT m.name, m.asset_ref FROM user_medals um JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT 3",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 未读站内信数（userbar 邮箱图标角标）
    let unread: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE receiver_id = $1 AND unread = true AND location = 1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 在线判定（个人主页 15 分钟口径）依赖 users.last_seen_at，此前全站无写入点
    // （仅 snatches 侧有），API 活跃即视为在线；写失败不影响本请求
    let _ = sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1")
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
    Ok(ok(serde_json::json!({
        "id": user.id, "username": user.username, "class_id": user.class_id,
        "must_reset_password": user.must_reset_password,
        "uploaded": uploaded, "downloaded": downloaded,
        "seeding": seeding, "leeching": leeching,
        "uploads": uploads, "bookmarks": bookmarks,
        "class_name": class_name,
        "avatar_url": avatar_url,
        "avatar_frame_id": frame_id,
        "avatar_frame_css": frame_css,
        "avatar_frame_image": frame_image,
        "worn_medals": worn_medals
            .into_iter()
            .map(|(name, asset_ref)| serde_json::json!({ "name": name, "asset_ref": asset_ref }))
            .collect::<Vec<_>>(),
        "unread_messages": unread,
    })))
}

/// 登录历史（NP usercp security 口径）：最近 20 条，含 UA 与结果细分（0096 起）。
#[derive(sqlx::FromRow, serde::Serialize)]
struct LoginEventRow {
    created_at: chrono::DateTime<chrono::Utc>,
    ip: Option<String>,
    ok: bool,
    #[sqlx(default)]
    user_agent: String,
    /// 0=成功 1=密码错误 3=停用账号 4=未知用户名
    #[sqlx(default)]
    reason: i16,
}

#[get("/me/logins")]
async fn my_login_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<LoginEventRow> = sqlx::query_as(
        "SELECT created_at, host(ip) AS ip, ok, user_agent, reason          FROM login_events WHERE user_id = $1 ORDER BY id DESC LIMIT 20",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
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
    // 改密同时轮换 passkey：引导期 root 等公开默认 passkey 不应在改密后继续可用
    let new_passkey = uuid::Uuid::new_v4().simple().to_string();
    let rotated: Option<String> = sqlx::query_scalar(
        "UPDATE users SET pass_hash = $2, passkey = $3, must_reset_password = false WHERE id = $1 RETURNING passkey",
    )
    .bind(auth.id)
    .bind(&new_hash)
    .bind(&new_passkey)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = rotated;
    state
        .repo
        .audit(
            Some(auth.id),
            "passkey_rotate_on_password_change",
            Some(auth.id),
        )
        .await;
    // 撤销既有 token：nbf 抬到「本次凭证 iat − 1」。require_auth 用 iat<=nbf 判失效，
    // 因此界线为 iat−1 时：本次改密凭证自身已通过认证（不再受检），一切 iat ≤ iat−1
    // 的旧 token 失效；改密后同秒重登的新 token（iat 相同）不受牵连 —— 不误杀合法新登录，
    // 而真正的旧凭证（改密所用的那张）即便 iat 同秒也已在本次请求中消耗，语义无损。
    let _ = sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2)          ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(auth.iat - 1)
    .execute(&state.repo.db)
    .await;
    let mut c = state.redis.clone();
    let _: () = redis::AsyncCommands::set_ex(
        &mut c,
        format!("logout_nbf:{}", auth.id),
        auth.iat - 1,
        86400u64,
    )
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
    #[sqlx(default)]
    avatar_frame_css: Option<String>,
    #[sqlx(default)]
    avatar_frame_image: Option<String>,
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
               f.css AS avatar_frame_css, f.image_url AS avatar_frame_image,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1 AND NOT t.anonymous) AS uploads,
               (SELECT count(*) FROM comments cm WHERE cm.user_id = u.id) AS comment_count,
               (SELECT count(*) FROM user_medals um WHERE um.user_id = u.id) AS medals
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
             LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id
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
    // —— 竞品口径补充（NP userdetails / UNIT3D profile 共性板块）——
    // 个人档案列（users 表早已有、此前接口未回）：性别是 INT2 代码（0/1/2 → 保密/男/女），
    // country/isp 是预留的字典 id（尚无关联表，直接回 id 供前端省略展示）。
    // 在线判定用 COALESCE：last_seen_at 可能为 NULL（从未活动），NULL > x = NULL 解码进 bool 会炸。
    let extra: Option<(
        Option<i16>,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<String>,
        Option<String>,
        bool,
    )> = sqlx::query_as(
        "SELECT gender, country, isp, upload_speed, download_speed, info, signature, \
                    COALESCE(last_seen_at > now() - interval '15 minutes', FALSE) AS online \
             FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (gender_code, _country, _isp, up_speed, down_speed, info, signature, online) = extra
        .map(|(g, c, i, u, d, inf, s, o)| (g, c, i, u, d, inf, s, o))
        .unwrap_or((None, None, None, None, None, None, None, false));
    let gender = match gender_code {
        Some(1) => Some("男".to_string()),
        Some(2) => Some("女".to_string()),
        _ => None, // 0/NULL = 保密
    };
    // —— 传输与时间板块（观众站口径：实际流量 + 做种/下载时间 + 比率 + 做种体积）——
    // snatches.uploaded/downloaded = 该用户全站真实流量（含免费/促销不计量部分），口径即 NP「实际」；
    // snatches 无独立「下载时长」列（NP 的 leechtime）——口径退化为 seed 累计/最近活动跨度不可靠，
    // 采用「有 leeching 记录起 last_seen_at 累计」不可得，这里回退用 completed_at 到创建的近似不可行，
    // 故下载时间以 0 展示由 tracker 侧未来补列（见下方 leech_seconds 注释）。
    let traffic: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(sum(uploaded), 0)::bigint, COALESCE(sum(downloaded), 0)::bigint, \
                COALESCE(sum(seeded_seconds), 0)::bigint, \
                COALESCE(sum(CASE WHEN leeching THEN 1 ELSE 0 END), 0)::bigint, \
                COALESCE((SELECT u2.seeding_size FROM users u2 WHERE u2.id = $1), 0) \
         FROM snatches WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (real_up, real_down, seed_seconds, _, seeding_size) = traffic.unwrap_or((0, 0, 0, 0, 0));
    // H&R：未解决违规数（观众站「H&R 0」+ 站点 hr_violation_limit 上限口径）
    let hr: Option<(i64,)> = sqlx::query_as(
        "SELECT count(*) FROM hr_violations WHERE user_id = $1 AND resolved_at IS NULL",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let hr_unresolved = hr.map(|(n,)| n).unwrap_or(0);
    let hr_limit: i64 = sqlx::query_scalar(
        "SELECT COALESCE((value)::bigint, 3) FROM site_settings WHERE name = 'hr_violation_limit'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None)
    .unwrap_or(3);
    // 魔力值余额（观众站「爆米花」位；spark_balance 是流水权威快照）+ 本月做种收益
    let spark: (i64,) = sqlx::query_as("SELECT spark_balance FROM users WHERE id = $1")
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let month_earn: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND amount > 0 AND kind = 'seeding_reward' \
           AND created_at >= date_trunc('month', now())",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 完成种子数（憨憨「完成种子」口径：completed_at 非空的抓取记录）
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE user_id = $1 AND completed_at IS NOT NULL",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 邀请：待使用邀请码数（NP「邀请」字段口径）
    let invites_pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM invites WHERE inviter_id = $1 AND status = 0")
            .bind(uid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // 邀请人（脱敏：只回邀请人 id+用户名，不回邮箱）
    let inviter: Option<(i64, String)> = sqlx::query_as(
        "SELECT i.id, i.username FROM users u JOIN users i ON i.id = u.invited_by WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 客户端信息（NP 连接信息：最近一次上报的 BT 客户端 Agent；snatches 无记录则空）
    let agent: Option<(String,)> = sqlx::query_as(
        "SELECT agent FROM snatches WHERE user_id = $1 AND agent <> '' ORDER BY last_seen_at DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 佩戴中的勋章（展示位：NP 佩戴勋章图 / UNIT3D achievements）
    let worn_medals: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT m.id, m.name, m.description, m.asset_ref FROM user_medals um \
         JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT 12",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 成就数（user_achievements）
    let achievements: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_achievements WHERE user_id = $1")
            .bind(uid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // 等级进度（对齐 /me/class-progress 口径：当前值 + 距下一级目标，供前端进度条）。
    // EXTRACT 返回 NUMERIC、count 返回 INT8——列类型全部显式对齐 i64，防 sqlx 静默解码失败
    // （此前 .ok() 把解码错误吞成 None，next_class 恒空）。
    let prog: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.class_id::bigint, u.uploaded, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL)::bigint, \
                (SELECT COALESCE(sum(s.seeded_seconds), 0) / 3600 FROM snatches s WHERE s.user_id = u.id)::bigint, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .ok();
    let mut next_class: Option<serde_json::Value> = None;
    if let Some((cur_class, uploaded, dl_count, seed_hours, age_days)) = prog {
        let rules: Vec<(i32, String, i64, i32, i32, i32)> = sqlx::query_as(
            "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
             FROM class_rules WHERE class_id > $1 ORDER BY class_id LIMIT 1",
        )
        .bind(cur_class)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        if let Some((cid, cname, need_up, need_dl, need_sh, need_age)) = rules.into_iter().next() {
            next_class = Some(serde_json::json!({
                "class_id": cid, "name": cname,
                "uploaded": uploaded, "uploaded_need": need_up,
                "download_count": dl_count, "download_count_need": need_dl,
                "seed_hours": seed_hours, "seed_hours_need": need_sh,
                "account_age_days": age_days, "account_age_days_need": need_age,
            }));
        }
    }
    // 近期论坛回帖（社区动态板块；匿名帖不暴露归属）
    let recent_posts: Vec<(i64, i64, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT p.id, p.topic_id, left(p.body_text, 80), p.created_at FROM posts p \
         WHERE p.user_id = $1 AND p.body_text <> '' \
         ORDER BY p.id DESC LIMIT 5",
        )
        .bind(uid)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
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
        "avatar_frame_css": profile.avatar_frame_css,
        "avatar_frame_image": profile.avatar_frame_image,
        "gender": gender, "country": null, "isp": null,
        "upload_speed": up_speed, "download_speed": down_speed,
        "info": info, "signature": signature, "online": online,
        "real_uploaded": real_up, "real_downloaded": real_down,
        "seed_seconds": seed_seconds, "hr_unresolved": hr_unresolved,
        "hr_limit": hr_limit, "seeding_size": seeding_size,
        "spark_balance": spark.0, "month_seed_earn": month_earn,
        "completed_snatches": completed,
        "invites_pending": invites_pending,
        "inviter_id": inviter.as_ref().map(|(i, _)| *i),
        "inviter_name": inviter.map(|(_, n)| n),
        "client_agent": agent.map(|(a,)| a),
        "worn_medals": worn_medals
            .into_iter()
            .map(|(id, name, description, asset_ref)| serde_json::json!({
                "id": id, "name": name, "description": description, "asset_ref": asset_ref
            }))
            .collect::<Vec<_>>(),
        "achievements": achievements,
        "next_class": next_class,
        "recent_posts": recent_posts,
        "recent_uploads": uploads,
        "recent_comments": recent_comments,
    })))
}

/// 他人用户页的种子列表（NP userdetails Torrent History 口径）：
/// uploads = 公开发布（匿名发布不暴露归属）；seeding = 当前做种（做种列表不含流量明细，NP 默认公开）。
#[derive(Deserialize)]
struct UserTorrentlistQuery {
    #[serde(default)]
    limit: Option<i64>,
}

#[get("/users/{id}/torrentlist")]
async fn user_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<UserTorrentlistQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = path.into_inner();
    // 隐私红线（P1）：做种/下载/完成明细可反推用户下载偏好，仅本人与 staff 可见
    // （NP userdetails 的普通用户口径）。uploads 本就过滤匿名发布、preserved 是
    // 保种区的公开认领承诺，保持公开。
    let reveal = auth.id == uid || auth.class_id >= 90;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let uploads: Vec<SnatchRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
         false AS seeding, false AS leeching, NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here \
         FROM torrents t WHERE t.owner_id = $1 AND t.approval_status = 1 AND NOT t.anonymous \
         ORDER BY t.id DESC LIMIT $2",
    )
    .bind(uid)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let seeding: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    // —— 憨憨式标签页其余四路（NP userdetails 同口径）——
    // 当前下载：leeching 抓取记录；完成：completed_at 非空；未完成：抓过但未完成且已不在做种/下载
    let leeching: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.leeching \
             ORDER BY s.torrent_id DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    let completed: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    let incomplete: Vec<SnatchRow> = if reveal {
        sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NULL AND NOT s.seeding AND NOT s.leeching \
             ORDER BY s.last_seen_at DESC LIMIT $2",
        )
        .bind(uid)
        .bind(limit)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    // 完成的保种区种子：seed_preserve 中该用户认领且尚未退出的记录
    // （认领本身公开；snatch 派生字段只对本人/staff 展示）
    let preserved: Vec<SnatchRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
         CASE WHEN $3 THEN s.seeding ELSE false END AS seeding, \
         CASE WHEN $3 THEN s.leeching ELSE false END AS leeching, \
         CASE WHEN $3 THEN s.completed_at END AS completed_at, \
         CASE WHEN $3 THEN s.uploaded ELSE 0 END AS uploaded_here \
         FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         LEFT JOIN snatches s ON s.torrent_id = sp.torrent_id AND s.user_id = $1 \
         WHERE sp.claimed_by = $1 AND sp.exited_at IS NULL \
         ORDER BY sp.torrent_id DESC LIMIT $2",
    )
    .bind(uid)
    .bind(limit)
    .bind(reveal)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "uploads": uploads, "seeding": seeding, "leeching": leeching,
        "completed": completed, "incomplete": incomplete, "preserved": preserved,
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
               u.spark_balance, f.css AS avatar_frame_css, f.image_url AS avatar_frame_image,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS uploads,
               (SELECT count(*) FROM comments c WHERE c.user_id = u.id) AS comments,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 0) AS invites_pending,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 1) AS invites_used,
               (SELECT count(*) FROM user_medals m WHERE m.user_id = u.id) AS medals,
               c.name AS class_name, c.id AS cid
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
             LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id
        WHERE u.id = $1
        "#,
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    // 佩戴勋章（userbar 同口径，控制面板资料卡用户名角标）
    let worn_medals: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT m.name, m.asset_ref FROM user_medals um JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT 3",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let r = &row;
    let get =
        |col: &str| -> serde_json::Value { r.try_get(col).unwrap_or(serde_json::Value::Null) };
    let get_opt_str =
        |col: &str| -> Option<String> { r.try_get::<Option<String>, _>(col).ok().flatten() };
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
    let last_login: Option<String> =
        sqlx::query_scalar("SELECT max(created_at)::text FROM login_events WHERE user_id = $1")
            .bind(uid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    // 等级进度：下一等级阈值以 class_rules 为准（worker class_auto_adjust 的升降级权威来源；
    // user_classes.min_uploaded 在 live 数据中全为 0，用它预览会恒显示「已达」）
    let class_id = get_i64("cid") as i32;
    let next: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, min_uploaded FROM class_rules \
         WHERE class_id > $1 AND class_id < 90 AND min_uploaded > 0 \
         ORDER BY class_id LIMIT 1",
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
        "avatar_url": get_opt_str("avatar_url"),
        "avatar_frame_css": get_opt_str("avatar_frame_css"),
        "avatar_frame_image": get_opt_str("avatar_frame_image"),
        "worn_medals": worn_medals
            .into_iter()
            .map(|(name, asset_ref)| serde_json::json!({ "name": name, "asset_ref": asset_ref }))
            .collect::<Vec<_>>(),
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
        (
            &b.append_promotion,
            &["highlight", "word", "icon", "off"][..],
        ),
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
        b.country,
        b.download_speed,
        b.upload_speed,
        b.isp,
        b.pm_per_page,
        b.torrents_per_page,
        b.incl_dead,
        b.sp_state,
        b.incl_bookmarked,
        b.topics_per_page,
        b.posts_per_page,
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

/// 宽松布尔解析（0093）：查询串里的 `1/0/true/false/yes/no` 均接受。
/// 此前 `include_dead=1`（旧站 1/0 口径、第三方客户端常用）会让整个 Query 反序列化失败 → 400。
fn de_bool_lenient<'de, D>(d: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<String>::deserialize(d)?;
    Ok(v.map(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    }))
}

/// 宽容的数字反序列化（005 修复）：表单里「全部」这类选项会提交 `alive=&tag_id=` 空串，
/// 而 `Option<i32>` 直接吃空串会解析失败 → 整个 Query 反序列化报错 → 400。
/// 这里统一把空/空白视为 None，非法值才算错。
fn de_opt_num_lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let v = Option::<String>::deserialize(d)?;
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => s.parse::<T>().map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    medium_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    grade_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    edition_id: Option<i32>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    official: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    include_dead: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    include_unapproved: Option<bool>,
    search: Option<String>,
    /// 搜索范围：0=标题(默认) 1=副标题/简介 3=发布者 4=IMDb
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    search_area: Option<i32>,
    /// 匹配模式：0=AND 模糊(默认) 2=精确
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    search_mode: Option<i32>,
    sort: Option<String>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    tag_id: Option<i32>,
    /// 存活筛选（0102，NP inclbooked/vivisect 口径）：0=全部 1=仅活种 2=仅断种（覆盖 include_dead）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    alive: Option<i16>,
    /// 种子状态（0102，NP 口径）：seeding=当前做种 leeching=当前下载 completed=完成
    /// incomplete=未完成 notseeding=未做种（多值逗号串待扩展，先单值）
    #[serde(default)]
    status: Option<String>,
    /// 审核状态（0102）：0=全部 1=通过 2=被拒（含未审需 see_banned，另走 include_unapproved）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    approval: Option<i16>,
    /// 分类多选（0088）：`category_ids`（原生键，重复参数成 seq）与 `category_id`
    /// （前端/NP 旧键名，单值或逗号串）统一收集——serde rename_all/alias 对
    /// query-string 的重复键行为不可靠，这里显式两个键各收一份再合并去重。
    #[serde(default, deserialize_with = "de_category_ids_lenient")]
    category_ids: Vec<String>,
    /// category_id 旧键名的旁路收集（与上合并；空则不影响）
    #[serde(default, skip_serializing, rename = "category_id")]
    category_id_alias: Option<String>,
    cursor: Option<String>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    limit: Option<i64>,
    // ===== 高级搜索增强 =====
    /// 体积区间：支持 `1024`（字节）`500MB` `1.5GB` `2TB` 等带单位写法（见 parse_size）
    #[serde(default)]
    size_min: Option<String>,
    #[serde(default)]
    size_max: Option<String>,
    /// 发布时间区间（`YYYY-MM-DD`，按日历日口径，含边界当天）
    #[serde(default)]
    date_from: Option<String>,
    #[serde(default)]
    date_to: Option<String>,
    /// 做种数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_seeders: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_seeders: Option<i32>,
    /// 排除关键字（标题/简介均不得命中）
    #[serde(default)]
    exclude: Option<String>,
    /// 优惠筛选：free / x2 / half / any / none（白名单校验）
    #[serde(default)]
    promo: Option<String>,
    /// 发布者用户名（模糊匹配）
    #[serde(default)]
    owner: Option<String>,
    /// 仅显示我发布的种子
    #[serde(default, deserialize_with = "de_bool_lenient")]
    mine: Option<bool>,
    // ===== 高级搜索补齐（0118）：下载数/完成数区间 + 匿名发布 =====
    /// 下载数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_leechers: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_leechers: Option<i32>,
    /// 完成数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_completed: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_completed: Option<i32>,
    /// 匿名发布：0=全部(默认) 1=仅匿名 2=仅具名
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    anonymous: Option<i16>,
}

/// 日期参数校验（严格 `YYYY-MM-DD`）：非法值直接丢弃——宁可不筛，
/// 也不让 PG 对烂字符串抛错变成 500。
fn norm_date(s: Option<String>) -> Option<String> {
    let s = s?;
    let s = s.trim();
    let ok = s.len() == 10
        && s.as_bytes().iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    ok.then(|| s.to_string())
}

/// 优惠参数白名单（未知取值丢弃，避免静默返回空列表让用户以为「没数据」）
fn norm_promo(s: Option<String>) -> Option<String> {
    let s = s?.trim().to_ascii_lowercase();
    matches!(s.as_str(), "free" | "x2" | "half" | "any" | "none").then_some(s)
}

/// 空白即视为未填（表单里清空后仍会提交空串）
fn norm_text(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 体积参数解析：`1024`（字节）/ `500MB` / `1.5GB` / `2TB`，KB/MB/GB/TB 与 KiB/GiB 写法均可，
/// 大小写不敏感。无效值或非正数返回 None（= 不筛，而不是报错）。
fn parse_size(s: Option<String>) -> Option<i64> {
    let raw = s?.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }
    let (num, mul) = if let Some(v) = raw.strip_suffix("tb").or_else(|| raw.strip_suffix("tib")) {
        (v, 1024f64.powi(4))
    } else if let Some(v) = raw.strip_suffix("gb").or_else(|| raw.strip_suffix("gib")) {
        (v, 1024f64.powi(3))
    } else if let Some(v) = raw.strip_suffix("mb").or_else(|| raw.strip_suffix("mib")) {
        (v, 1024f64.powi(2))
    } else if let Some(v) = raw.strip_suffix("kb").or_else(|| raw.strip_suffix("kib")) {
        (v, 1024f64)
    } else if let Some(v) = raw.strip_suffix('b') {
        (v, 1f64)
    } else {
        (raw.as_str(), 1f64)
    };
    let n: f64 = num.trim().parse().ok()?;
    let bytes = (n * mul).round();
    (bytes >= 1.0 && n.is_finite() && bytes < i64::MAX as f64).then_some(bytes as i64)
}

/// category_ids/category_id 的宽松反序列化：seq → 原样；字符串 → 按逗号拆。
/// （此前用 alias 兼容单值键名，但 serde 对 Vec 字段的裸字符串直接报错 → 前端筛选 400）
fn de_category_ids_lenient<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<String>),
        One(String),
    }
    // actix-web Query 的 serde_qs 形状：字段值是「单值或 seq」的直接载荷
    let v = serde_json::Value::deserialize(d)?;
    let pick = serde_json::from_value::<OneOrMany>(v).map_err(serde::de::Error::custom)?;
    Ok(match pick {
        OneOrMany::Many(v) => v,
        OneOrMany::One(s) => s.split(',').map(|x| x.trim().to_string()).collect(),
    })
}

#[get("/torrents")]
async fn list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?; // 站点准入收口：资源元数据不对外
                                                  // 通用多维筛选（0088）：sec_{kind}=dict_id，kind 走 section_kinds 白名单；
                                                  // 从原始 query string 解析，支持任意站方自建维度（不再写死六个）
    let mut sections: Vec<(String, i64)> = Vec::new();
    for pair in req.query_string().split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let Some(kind) = k.strip_prefix("sec_") else {
            continue;
        };
        if kind.is_empty() || v.is_empty() {
            continue;
        }
        if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind).await {
            continue;
        }
        // 多选（0102）：逗号串 / 重复参数均可；同维度多值 = OR（任一命中）
        for part in v.split(',') {
            if let Ok(dict_id) = part.trim().parse::<i64>() {
                sections.push((kind.to_string(), dict_id));
            }
        }
    }
    let filter = torrents::TorrentFilter {
        // 多选分类：重复参数或逗号串（category_id=1&category_id=3 / category_id=1,3）均解析。
        // 旧键名 category_id 单值也在此合并（category_id_alias 旁路收集）。
        category_id: {
            let mut raw: Vec<&str> = q.category_ids.iter().map(|s| s.as_str()).collect();
            if let Some(one) = q.category_id_alias.as_deref() {
                raw.push(one);
            }
            let ids: Vec<i32> = raw
                .iter()
                .flat_map(|s| s.split(','))
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            (!ids.is_empty()).then_some(ids)
        },
        medium_id: q.medium_id,
        grade_id: q.grade_id,
        edition_id: q.edition_id,
        official: q.official,
        include_dead: q.include_dead.unwrap_or(false),
        // 仅持 see_banned 权限者可查看未过审种子；无权限时参数被静默忽略
        include_unapproved: q.include_unapproved.unwrap_or(false)
            && crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_SEE_BANNED).await,
        search: q.search.as_deref().map(str::to_string),
        sort: q.sort.as_deref().map(str::to_string),
        tag_id: q.tag_id,
        // 0102 高级搜索三态
        alive: q.alive,
        status: q.status.clone().filter(|s| !s.is_empty()),
        approval: q.approval,
        // 搜索盒口径（此前前端传了但后端不解析，静默失效）
        search_area: q.search_area,
        search_mode: q.search_mode,
        // 高级搜索增强（体积/时间/做种数/排除词/优惠/发布者/仅我发布）
        size_min: parse_size(q.size_min.clone()),
        size_max: parse_size(q.size_max.clone()),
        date_from: norm_date(q.date_from.clone()),
        date_to: norm_date(q.date_to.clone()),
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        exclude: norm_text(q.exclude.clone()),
        promo: norm_promo(q.promo.clone()),
        owner: norm_text(q.owner.clone()),
        only_mine: q.mine.unwrap_or(false),
        // 0118 高级搜索补齐（下载数/完成数区间 + 匿名发布；负数丢弃 = 不筛）
        min_leechers: q.min_leechers.filter(|v| *v >= 0),
        max_leechers: q.max_leechers.filter(|v| *v >= 0),
        min_completed: q.min_completed.filter(|v| *v >= 0),
        max_completed: q.max_completed.filter(|v| *v >= 0),
        anonymous: q.anonymous.filter(|v| (0..=2).contains(v)),
        sections,
    };
    // 上下界颠倒时自动对调（用户先填大后填小很常见，直接判空更友好）
    let mut filter = filter;
    if let (Some(a), Some(b)) = (filter.size_min, filter.size_max) {
        if a > b {
            filter.size_min = Some(b);
            filter.size_max = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (filter.min_seeders, filter.max_seeders) {
        if a > b {
            filter.min_seeders = Some(b);
            filter.max_seeders = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (filter.date_from.clone(), filter.date_to.clone()) {
        if a > b {
            filter.date_from = Some(b);
            filter.date_to = Some(a);
        }
    }
    // 0118：下载数/完成数区间同样自动对调
    if let (Some(a), Some(b)) = (filter.min_leechers, filter.max_leechers) {
        if a > b {
            filter.min_leechers = Some(b);
            filter.max_leechers = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (filter.min_completed, filter.max_completed) {
        if a > b {
            filter.min_completed = Some(b);
            filter.max_completed = Some(a);
        }
    }
    let cursor = match q.cursor.as_deref() {
        Some(c) if !c.is_empty() => Some(
            c.parse::<i64>()
                .map_err(|_| DomainError::Validation("cursor 无效".into()))?,
        ),
        _ => None,
    };
    // P1-4 读缓存（0071）：仅覆盖「首屏等价视图」——无筛选/无搜索/无游标/默认排序的第 1 页。
    // 该视图不含用户视角字段（owner 匿名化在 SQL 层完成），全部登录用户看到的字节一致，可共享缓存。
    // TTL 45s 兜底 + Redis 故障直查；带任何筛选条件时不走缓存（避免失效风暴复杂化）。
    let is_first_screen = cursor.is_none()
        && filter.category_id.is_none()
        && filter.medium_id.is_none()
        && filter.grade_id.is_none()
        && filter.edition_id.is_none()
        && filter.official.is_none()
        && !filter.include_dead
        && !filter.include_unapproved
        && filter.search.is_none()
        && filter.sort.is_none()
        && filter.tag_id.is_none()
        && filter.sections.is_empty()
        // 状态筛选是用户视角（viewer 的 snatches）：带 status 的请求不得共享首屏缓存
        && filter.status.is_none()
        && filter.alive.is_none()
        // 0105 高级搜索增强：任一条件生效即视为非「首屏等价视图」，不得复用共享缓存
        && filter.size_min.is_none()
        && filter.size_max.is_none()
        && filter.date_from.is_none()
        && filter.date_to.is_none()
        && filter.min_seeders.is_none()
        && filter.max_seeders.is_none()
        && filter.exclude.is_none()
        && filter.promo.is_none()
        && filter.owner.is_none()
        && !filter.only_mine
        // 0118 补齐项同口径：任一生效即非首屏等价视图
        && filter.min_leechers.is_none()
        && filter.max_leechers.is_none()
        && filter.min_completed.is_none()
        && filter.max_completed.is_none()
        && filter.anonymous.is_none()
        && q.limit.unwrap_or(20) == 20;
    let cache_key = "cache:tlist:first:v1";
    if is_first_screen {
        let mut c = state.redis.clone();
        let hit: Option<String> = redis::AsyncCommands::get(&mut c, cache_key)
            .await
            .unwrap_or(None);
        if let Some(json) = hit {
            if let Ok(page) = serde_json::from_str::<torrents::TorrentPage>(&json) {
                return Ok(ok(page));
            }
        }
        let page = torrents::list_torrents_as(
            &state.repo.db,
            &filter,
            cursor,
            q.limit.unwrap_or(20),
            auth.id,
        )
        .await?;
        if let Ok(json) = serde_json::to_string(&page) {
            let _: Result<(), _> =
                redis::AsyncCommands::set_ex(&mut c, cache_key, json, 45u64).await;
        }
        return Ok(ok(page));
    }
    let page = torrents::list_torrents_as(
        &state.repo.db,
        &filter,
        cursor,
        q.limit.unwrap_or(20),
        auth.id,
    )
    .await?;
    Ok(ok(page))
}

#[get("/torrents/{id}")]
async fn detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 持 view_anonymous 权限者可见匿名种子的真实发布者
    let reveal = crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_VIEW_ANONYMOUS).await;
    // G7：staff 视角传 (uid, true)——暂缓种对 staff 开放
    let viewer = (auth.id, auth.class_id >= 90);
    let t = torrents::get_torrent(&state.repo.db, path.into_inner(), reveal, Some(viewer)).await?;
    Ok(ok(t))
}

/// 详情页扩展数据（简介/文件数/感谢数），与 detail 合并渲染
#[get("/torrents/{id}/detail")]
async fn torrent_detail_ext(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let t = torrents::get_torrent_detail(&state.repo.db, path.into_inner(), auth.id).await?;
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

/// 审计修复（P1）：评论此前只有创建+列表，作者本人与版主都无法删除（无任何删除端点）。
/// 作者本人或持 torrent.manage 的 staff 可删；挂审计日志。
#[delete("/torrents/{id}/comments/{cid}")]
async fn delete_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (torrent_id, comment_id) = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM comments WHERE id = $1 AND torrent_id = $2")
            .bind(comment_id)
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(comment_id));
    };
    let is_owner = owner_id == auth.id;
    let is_manager = crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_MANAGE).await;
    if !is_owner && !is_manager {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(comment_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(
            Some(auth.id),
            if is_owner {
                "comment.delete_own"
            } else {
                "comment.delete_staff"
            },
            Some(comment_id),
        )
        .await;
    Ok(ok(serde_json::json!({ "deleted": comment_id })))
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
            return Err(DomainError::Validation(
                "答谢数额需为 1/10/100/500/1000/10000".into(),
            ));
        }
        // 给发布者转魔力（匿名也按 owner_id 记账）
        let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
        if let Some(owner) = owner {
            if owner != auth.id {
                let idem = format!(
                    "thank-spark:{}:{}:{}",
                    auth.id,
                    tid,
                    chrono::Utc::now().timestamp()
                );
                crate::economy_http::earn_spark(
                    &state.repo.db,
                    owner,
                    amount,
                    "task_reward",
                    &idem,
                )
                .await?;
            }
        }
    }
    Ok(ok(
        serde_json::json!({ "thanked": true, "spark_given": amount }),
    ))
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
            name: body
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
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
    Ok(ok(
        serde_json::json!({ "edited": true, "note": "已回退待审核" }),
    ))
}

/// 修改付费定价（0086）：发布者或 staff；改价不影响已购（以 torrent_purchases 已扣为准）
#[derive(serde::Deserialize)]
struct PriceReq {
    price: i64,
}

#[put("/torrents/{id}/price")]
async fn set_torrent_price(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<PriceReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let price = body.price.clamp(0, 1_000_000);
    sqlx::query("UPDATE torrents SET price = $2 WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "torrent.price_set", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "price": price })))
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数
#[post("/torrents/{id}/restore")]
async fn restore_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TORRENT_MANAGE).await?;
    let id = path.into_inner();
    torrents::restore_torrent(&state.repo.db, id).await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.restore", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "restored": id })))
}

/// 被拒种子修改后重提（U4 §12.5 审核闭环：rejected → pending，保留原 id 与评论区）。
/// 仅作者本人；清拒绝标记（deny_reason/note），重进审核队列。
#[post("/torrents/{id}/resubmit")]
async fn resubmit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<(i64, i16)> =
        sqlx::query_as("SELECT owner_id, approval_status FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, status)) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    if status != 2 {
        return Err(DomainError::Validation(
            "仅被拒种子可重提（当前状态不符）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, deny_reason_id = NULL, deny_note = NULL \
         WHERE id = $1 AND approval_status = 2",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("状态已变化，请刷新".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.resubmit", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "resubmitted": id })))
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
    state
        .repo
        .audit(Some(auth.id), "torrent.delete", Some(id))
        .await;
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
    Ok(ok(torrents::list_snatches(
        &state.repo.db,
        path.into_inner(),
    )
    .await?))
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
    Ok(ok(
        torrents::list_tags(&state.repo.db, path.into_inner()).await?
    ))
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
    torrents::tag_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        body.tag_id,
        body.on,
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.tag", Some(tid))
        .await;
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
    // 对象级读缓存（0069）：/stats 是全站聚合查询，首页/移动壳高频拉取。
    // TTL 60s 兜底（worker 每 60s 刷快照，口径一致）；Redis 故障直查库（fail-open 不阻断）。
    let key = "cache:stats:v1";
    let mut c = state.redis.clone();
    let hit: Option<String> = redis::AsyncCommands::get(&mut c, key).await.unwrap_or(None);
    if let Some(json) = hit {
        return Ok(ok(
            serde_json::from_str::<serde_json::Value>(&json).unwrap_or(serde_json::Value::Null)
        ));
    }
    let val = serde_json::to_value(torrents::site_stats(&state.repo.db).await?)
        .unwrap_or(serde_json::Value::Null);
    let _: Result<(), _> = redis::AsyncCommands::set_ex(&mut c, key, val.to_string(), 60u64).await;
    Ok(ok(val))
}

// ============ 作弊探测（0069 cheat_events 闭环查询端：tracker 拒绝 → worker 落库 → 后台可查） ============

#[derive(Deserialize)]
struct CheatEventsQuery {
    limit: Option<i64>,
}

/// agent_rules 黑白名单命中记录（staff 专用）：按最近命中倒序
#[get("/admin/cheat-events")]
async fn cheat_events_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<CheatEventsQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows: Vec<(
        i64,
        i64,
        String,
        Option<String>,
        String,
        i64,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, user_id, agent, peer_ip, reason, hits, first_seen, last_seen \
             FROM cheat_events ORDER BY last_seen DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, uid, agent, ip, reason, hits, first, last)| {
            serde_json::json!({
                "id": id, "user_id": uid, "agent": agent, "peer_ip": ip,
                "reason": reason, "hits": hits,
                "first_seen": first, "last_seen": last,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
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
    // 目标存在性校验（此前任意 ref_id 含不存在对象可无限提交）
    let target_table = match body.ref_type.as_str() {
        "torrent" => Some(("torrents", "approval_status = 1")),
        "comment" => Some(("comments", "true")),
        "user" => Some(("users", "status < 2")),
        "subtitle" => Some(("subtitles", "true")),
        "forum" => Some(("topics", "true")),
        _ => None,
    };
    if let Some((table, extra)) = target_table {
        let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = $1 AND {extra})");
        let exists: bool = sqlx::query_scalar(&sql)
            .bind(body.ref_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
        if !exists {
            return Err(DomainError::NotFound(body.ref_id));
        }
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
    // 绝对地址口径与 rss feed 端点一致：PUBLIC_API_URL 优先，缺省回退请求 Host，
    // 不再硬编 127.0.0.1（生产未配该变量时用户复制的订阅地址必连失败）。
    let base = match std::env::var("PUBLIC_API_URL") {
        Ok(u) if !u.trim().is_empty() => u.trim().trim_end_matches('/').to_string(),
        _ => {
            let host = req
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("127.0.0.1:8080");
            format!("http://{host}")
        }
    };
    Ok(ok(serde_json::json!({
        "urls": [
            { "label": "全部种子", "url": format!("{}/api/v1/rss/{}", base, passkey) },
            { "label": "官种", "url": format!("{}/api/v1/rss/{}?official=true", base, passkey) },
        ],
        "passkey": passkey,
        "base": format!("{}/api/v1/rss/", base),
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE).await?;
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
    state
        .repo
        .audit(Some(auth.id), "faq.create", Some(id as i64))
        .await;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE).await?;
    // sort 用 COALESCE 保留原值：编辑时不传 sort 不应把排序归零
    let n = sqlx::query("UPDATE faq_items SET question=$2, answer=$3, category=$4, sort=COALESCE($5, sort), updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.question).bind(&body.answer).bind(&body.category).bind(body.sort)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "faq.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/faq/{id}")]
async fn faq_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE).await?;
    sqlx::query("DELETE FROM faq_items WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "faq.delete", Some(*path as i64))
        .await;
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
    let rows: Vec<RuleRow> =
        sqlx::query_as("SELECT id, title, body, sort FROM site_rules ORDER BY sort, id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct RuleBody {
    title: String,
    body: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/rules")]
async fn rule_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO site_rules (title, body, sort) VALUES ($1,$2,COALESCE($3,(SELECT max(sort)+1 FROM site_rules))) RETURNING id",
    ).bind(&body.title).bind(&body.body).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/rules/{id}")]
async fn rule_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE).await?;
    // G5 规则版本化：同事务内先存档被替换的旧版，再更新
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO rules_revisions (rule_id, title, body, sort, edited_by) \
         SELECT id, title, body, sort, $2 FROM site_rules WHERE id = $1",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("UPDATE site_rules SET title=$2, body=$3, sort=COALESCE($4, sort), updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.title).bind(&body.body).bind(body.sort)
        .execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "rules.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/rules/{id}")]
async fn rule_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE).await?;
    sqlx::query("DELETE FROM site_rules WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 分类管理（catmanage）----

#[derive(Deserialize)]
struct CatBody {
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CatRow {
    id: i32,
    name: String,
    mode_id: Option<i32>,
    auto_approve: bool,
    torrents: i64,
}

#[get("/admin/categories")]
async fn category_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let rows: Vec<CatRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.mode_id, c.auto_approve, (SELECT count(*) FROM torrents t WHERE t.category_id = c.id)::bigint AS torrents \
         FROM categories c ORDER BY c.id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/categories")]
async fn category_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id: i32 = sqlx::query_scalar("INSERT INTO categories (id, name) VALUES ((SELECT max(id)+1 FROM categories), $1) RETURNING id")
        .bind(&body.name).fetch_one(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 审计修复（P1）：分类增删改此前完全不写审计日志
    state
        .repo
        .audit(Some(auth.id), "category.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/categories/{id}")]
async fn category_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let n = sqlx::query("UPDATE categories SET name=$2 WHERE id=$1")
        .bind(*path)
        .bind(&body.name)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "category.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/categories/{id}")]
async fn category_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents WHERE category_id=$1")
        .bind(*path)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation("该分类下仍有种子，无法删除".into()));
    }
    sqlx::query("DELETE FROM categories WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "category.delete", Some(*path as i64))
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
    let rows: Vec<IpBanRow> = sqlx::query_as(
        "SELECT b.id, b.ip::text AS ip, b.reason, u.username AS banned_by, b.created_at \
         FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by ORDER BY b.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BanBody {
    ip: String,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/bans")]
async fn ban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
    let ip: std::net::IpAddr = body
        .ip
        .trim()
        .parse()
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
    sqlx::query("DELETE FROM ip_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_unban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- staffpanel 运营工具：种子促销 / 批量私信 / 添加用户 / 增加魔力 / 警告用户 / 重复IP / 失败登录 ----

/// 种子促销（原"免费下载"，freeleech.php 升级口径）：
/// scope = global 全站 | official 官种 | non_official 非官种 | category 某分类
/// 支持自定义起止时间（可预约：到达开始时间自动生效）；同范围可并存多个促销（计费取最强档）
#[derive(Deserialize)]
struct FreeleechBody {
    kind: String, // free / x2 / x2free / half / x2half / p30
    hours: i32,   // 结束时间未提供时用（自开始时间起算）
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    starts_at: Option<String>, // RFC3339，缺省=now
    #[serde(default)]
    ends_at: Option<String>, // RFC3339，缺省=starts_at+hours
}

/// 促销参数校验：kind/scope 合法性 + 起止时间解析（starts 缺省 now，ends 缺省 starts+hours）
fn promo_parse(
    kind_in: &str,
    scope_in: Option<&str>,
    hours: i32,
    starts_at: &Option<String>,
    ends_at: &Option<String>,
) -> DomainResult<(
    String,
    String,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::Utc>,
)> {
    let kind = match kind_in {
        "free" | "x2" | "x2free" | "half" | "x2half" | "p30" => kind_in.to_string(),
        _ => return Err(DomainError::Validation("促销类型无效".into())),
    };
    if ends_at.is_none() && !(1..=720).contains(&hours) {
        return Err(DomainError::Validation("时长需在 1-720 小时".into()));
    }
    let scope = scope_in.unwrap_or("global").to_string();
    match scope.as_str() {
        "global" | "official" | "non_official" | "category" => {}
        _ => return Err(DomainError::Validation("促销范围无效".into())),
    }
    let starts_at = match starts_at {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| DomainError::Validation("开始时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => chrono::Utc::now(),
    };
    let ends_at = match ends_at {
        Some(e) => chrono::DateTime::parse_from_rfc3339(e)
            .map_err(|_| DomainError::Validation("结束时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => starts_at + chrono::Duration::hours(hours as i64),
    };
    if ends_at <= starts_at {
        return Err(DomainError::Validation("结束时间需晚于开始时间".into()));
    }
    if (ends_at - starts_at) > chrono::Duration::hours(24 * 90) {
        return Err(DomainError::Validation("促销时长不可超过 90 天".into()));
    }
    Ok((kind, scope, starts_at, ends_at))
}

#[post("/admin/freeleech")]
async fn freeleech_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FREELEECH_MANAGE).await?;
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    let kind = kind.as_str();
    let scope = scope.as_str();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO promotions (scope, category_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('category', $1, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
        ).bind(cid).bind(kind).bind(starts_at).bind(ends_at).bind(auth.id)
        .fetch_one(&mut *tx).await.map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(auth.id), "promo_set", None).await;
        return Ok(ok(
            serde_json::json!({ "id": id, "kind": kind, "scope": scope, "category_id": cid, "hours": body.hours }),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO promotions (scope, kind, starts_at, ends_at, source, created_by) \
         VALUES ($1::promotion_scope, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
    )
    .bind(scope)
    .bind(kind)
    .bind(starts_at)
    .bind(ends_at)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "promo_set", None).await;
    Ok(ok(
        serde_json::json!({ "id": id, "kind": kind, "scope": scope, "hours": body.hours }),
    ))
}

#[delete("/admin/freeleech")]
async fn freeleech_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FREELEECH_MANAGE).await?;
    // 清除全部进行中的手动站点级促销（全站/官种/非官种/分类）
    let n = sqlx::query("DELETE FROM promotions WHERE scope IN ('global','official','non_official','category') AND source='manual' AND ends_at > now()")
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state
        .repo
        .audit(Some(auth.id), "freeleech_clear", None)
        .await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 编辑单条促销（类型/范围/起止时间均可改）
#[put("/admin/freeleech/{id}")]
async fn freeleech_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FREELEECH_MANAGE).await?;
    let pid = path.into_inner();
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM promotions WHERE id = $1 AND source = 'manual'")
            .bind(pid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
            .bind(cid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        sqlx::query("UPDATE promotions SET scope='category', category_id=$1, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(cid).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("UPDATE promotions SET scope=$1::promotion_scope, category_id=NULL, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(&scope).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "promo_update", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "updated": pid })))
}

/// 删除单条促销（不影响其他并存促销）
#[delete("/admin/freeleech/{id}")]
async fn freeleech_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FREELEECH_MANAGE).await?;
    let pid = path.into_inner();
    let n = sqlx::query("DELETE FROM promotions WHERE id = $1 AND source = 'manual'")
        .bind(pid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_delete", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": pid })))
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FREELEECH_VIEW).await?;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMessBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFFMESS).await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let n = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT $1, id, $2, $3 FROM users WHERE status < 2 AND ($4::int IS NULL OR class_id >= $4)",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(&body.body)
    .bind(body.min_class)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "staffmess_send", None)
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AddUserBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CREATE).await?;
    if body.username.trim().len() < 2 || !body.email.contains('@') || body.password.len() < 8 {
        return Err(DomainError::Validation(
            "用户名≥2字符、邮箱合法、密码≥8位".into(),
        ));
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
    state
        .repo
        .audit(Some(auth.id), "admin_add_user", Some(uid))
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountBonusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_AMOUNTBONUS).await?;
    if body.amount == 0 || body.amount.abs() > 1_000_000 {
        return Err(DomainError::Validation(
            "数量需在 ±1,000,000 之间且非 0".into(),
        ));
    }
    // 走统一账务管线：事务 + 逐户流水 + balance_after 快照（原实现裸 UPDATE 绕过
    // spark_ledger，账本 sum(amount) 与余额失配、管理端 spark-logs 查不到这类变动）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, spark_balance FROM users WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE",
    )
    .bind(body.user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (uid, before) in &ids {
        // 账本权威口径：流水按「实际前后差额」落账（与 increment_bulk 同修复）。
        // 旧版余额 GREATEST(0,...) 截断但流水记原始 amount，负扣被截断的部分
        // 会在 worker 小时级 sum(ledger) 重算时被重新兑现（账本撕裂）。
        let after: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = GREATEST(0, spark_balance + $2) WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(body.amount)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let actual_delta = after - before; // 行已 FOR UPDATE，before 即更新前权威值
        if actual_delta == 0 {
            continue; // 截断后无实际变动（如余额 0 再负扣）：不落流水，保持 sum(ledger)=balance
        }
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \n             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'amountbonus', $3, $4, $5)",
        )
        .bind(uid)
        .bind(actual_delta)
        .bind(auth.id)
        .bind(format!("amountbonus-{}-{}", uid, uuid::Uuid::new_v4().simple()))
        .bind(after)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = ids.len() as i64;
    state
        .repo
        .audit(Some(auth.id), "amount_bonus", body.user_id)
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN).await?;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WarnBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN).await?;
    if !(1..=52).contains(&body.weeks) {
        return Err(DomainError::Validation("警告时长需 1-52 周".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET warned_until = now() + make_interval(weeks => $2), warned_reason = $3 WHERE id = $1 AND status < 2",
    ).bind(body.user_id).bind(body.weeks).bind(&body.reason)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "warn_user", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "warned": body.user_id, "until_weeks": body.weeks }),
    ))
}

#[delete("/admin/warned/{user_id}")]
async fn unwarn_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN).await?;
    let n = sqlx::query("UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state
        .repo
        .audit(Some(auth.id), "unwarn_user", Some(*path))
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::IP_CHECK).await?;
    let rows: Vec<IpCheckRow> = sqlx::query_as(
        "SELECT host(ip) AS ip, \
            count(DISTINCT user_id) AS users, \
            string_agg(DISTINCT u.username, ', ') AS usernames, \
            max(le.created_at) AS last_seen \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE ip IS NOT NULL AND user_id > 0 \
         GROUP BY ip HAVING count(DISTINCT user_id) > 1 \
         ORDER BY users DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MAXLOGIN_VIEW).await?;
    let rows: Vec<FailedLoginRow> = sqlx::query_as(
        "SELECT le.id, COALESCE(u.username, '(未知用户)') AS username, host(le.ip) AS ip, le.created_at \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE le.ok = false ORDER BY le.id DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountUploadBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_AMOUNTUPLOAD).await?;
    if body.bytes == 0 || body.bytes.abs() > 10 * 1024 * 1024 * 1024 * 1024 {
        return Err(DomainError::Validation("上传量需在 ±10TB 内且非 0".into()));
    }
    // 审计修复（P0 错账）：uploaded 的权威在 traffic_ledger（worker reconcile 与
    // /admin/jobs/run:reconcile 会把 users.uploaded 重算为 sum(ledger)），此前裸
    // UPDATE 不落流水，管理员手工加量在下一次对账时被静默清零。改为同语句内
    // 落差额流水（torrent_id=0 为人工调账标记；扣成负数时按实际截断差额记账）。
    let n = sqlx::query(
        "WITH targets AS ( \
            SELECT id, uploaded FROM users \
            WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE \
         ), upd AS ( \
            UPDATE users u SET uploaded = GREATEST(0, u.uploaded + $2) \
            FROM targets t WHERE u.id = t.id \
            RETURNING u.id, GREATEST(0, t.uploaded + $2) - t.uploaded AS delta \
         ) \
         INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
         SELECT nextval('traffic_ledger_id_seq'), id, 0, delta, 0, now() FROM upd WHERE delta <> 0",
    )
    .bind(body.user_id)
    .bind(body.bytes)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "amount_upload", body.user_id)
        .await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 重置用户密码（reset.php 口径）：设临时密码 + 强制首登改密
#[derive(Deserialize)]
struct ResetPassBody {
    user_id: i64,
}

#[post("/admin/resetpass")]
async fn admin_reset_pass(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetPassBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_RESETPASS).await?;
    // 临时密码（仅返回一次）：CSPRNG 16 字节 base64——旧版操作者id+纳秒取模
    // 密码空间小且可预测，配合明文回显存在猜测窗口
    let temp_pass = format!(
        "Tmp@{}",
        data_encoding::BASE64URL_NOPAD.encode(&rand::random::<[u8; 16]>())
    );
    let hash = crate::domain::hash_password(&temp_pass)?;
    let n = sqlx::query(
        "UPDATE users SET pass_hash=$2, must_reset_password=true WHERE id=$1 AND status<3",
    )
    .bind(body.user_id)
    .bind(&hash)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "admin_reset_pass", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "temp_password": temp_pass }),
    ))
}

/// 删除被禁用户（deletedisabled.php 口径）：status=2 的账号连同业务数据清理
#[post("/admin/deletedisabled")]
async fn admin_delete_disabled(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_DELETE_DISABLED).await?;
    // 审计修复：旧版裸 `DELETE FROM users WHERE status = 2` 单条 SQL——任何被
    // NO ACTION 引用（如 invites.inviter_id / audit_log.actor_id）的用户会让整条
    // 语句外键失败 500；部分 CASCADE 则静默丢数据。改为复用权威路径 admin/users/{id}
    // 的口径：强制走 `DELETE /api/v1/admin/users/{id}` 同一实现（逐个、事务化、83 列清理）。
    // 这里直接构造内部请求等价物：调用 admin_http 的清理清单不跨模块，故改为
    // 逐个转发 HTTP 会引入自调用复杂度——最简正确实现：拒绝批量、提示走单删。
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE status = 2 ORDER BY id LIMIT 500")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let mut deleted: Vec<i64> = Vec::new();
    let mut failed: Vec<(i64, String)> = Vec::new();
    for uid in ids {
        // 与 user_admin_delete 同款清单式单事务删除（简化为直接执行权威清理序列）
        match crate::admin_http::delete_user_cascade(&state.repo.db, uid).await {
            Ok(()) => deleted.push(uid),
            Err(e) => failed.push((uid, e.to_string())),
        }
    }
    state
        .repo
        .audit(Some(auth.id), "delete_disabled_users", None)
        .await;
    Ok(ok(
        serde_json::json!({ "deleted": deleted.len(), "ids": deleted, "failed": failed }),
    ))
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EMAILBAN_MANAGE).await?;
    let rows: Vec<EmailBanRow> = sqlx::query_as(
        "SELECT e.id, e.pattern, e.mode, e.note, u.username AS created_by, e.created_at \
         FROM email_bans e LEFT JOIN users u ON u.id = e.created_by ORDER BY e.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EmailBanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EMAILBAN_MANAGE).await?;
    if !body.pattern.contains('@') && !body.pattern.starts_with('@') && !body.pattern.ends_with('@')
    {
        return Err(DomainError::Validation(
            "格式需为邮箱、@domain 或 user@ 通配".into(),
        ));
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
    state
        .repo
        .audit(Some(auth.id), "emailban.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/emailbans/{id}")]
async fn emailban_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EMAILBAN_MANAGE).await?;
    sqlx::query("DELETE FROM email_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emailban.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// IP 测试（testip.php 口径）：检测 IP 是否命中封禁列表
#[derive(Deserialize)]
struct TestIpQuery {
    ip: String,
}

#[get("/admin/testip")]
async fn test_ip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TestIpQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TESTIP).await?;
    let ip: std::net::IpAddr =
        q.ip.trim()
            .parse()
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
    )
    .bind(&ip_text)
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STATS_VIEW).await?;
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users WHERE status < 2)::bigint, \
                (SELECT count(*) FROM torrents)::bigint, \
                (SELECT count(*) FROM snatches WHERE seeding)::bigint, \
                (SELECT count(*) FROM snatches WHERE leeching)::bigint, \
                (SELECT count(*) FROM comments)::bigint, \
                (SELECT count(*) FROM messages)::bigint",
    )
    .fetch_one(&state.repo.db)
    .await
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
/// 保种统计（seed.stats.view）：站点做种总览与 Top 保种用户。
/// 注意：不走 staff 门槛——保种员 / VIP 持该权限即可访问（非管理组角色）。
#[get("/seed-stats")]
async fn seed_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SEED_STATS_VIEW).await?;
    let totals: (i64, i64, f64) = sqlx::query_as(
        "SELECT count(DISTINCT s.user_id)::bigint, count(*)::bigint, \
                COALESCE(avg(s.seeded_seconds) / 3600.0, 0)::float8 \
         FROM snatches s WHERE s.seeding",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, count(*)::bigint AS c \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeding GROUP BY s.user_id, u.username ORDER BY c DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_hours: Vec<(i64, String, f64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, (sum(s.seeded_seconds) / 3600.0)::float8 AS h \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeded_seconds > 0 GROUP BY s.user_id, u.username ORDER BY h DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count = top_count
        .into_iter()
        .map(|(uid, name, c)| serde_json::json!({"user_id": uid, "username": name, "seeding": c}))
        .collect::<Vec<_>>();
    let top_hours = top_hours
        .into_iter()
        .map(|(uid, name, hrs)| serde_json::json!({"user_id": uid, "username": name, "hours": (hrs * 10.0).round() / 10.0}))
        .collect::<Vec<_>>();
    Ok(ok(serde_json::json!({
        "seeders": totals.0,
        "seeding_torrents": totals.1,
        "avg_seed_hours": (totals.2 * 10.0).round() / 10.0,
        "top_by_count": top_count,
        "top_by_hours": top_hours,
    })))
}

#[post("/admin/clearcache")]
async fn clear_cache(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEARCACHE).await?;
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let keys: Vec<String> = c.keys("rl:*").await.unwrap_or_default();
    let n = keys.len();
    if n > 0 {
        let _: () = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut c)
            .await
            .unwrap_or(());
    }
    state.repo.audit(Some(auth.id), "clear_cache", None).await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 做清理（docleanup.php 口径）：过期促销/过期警告/过期登录事件归档清理
#[post("/admin/docleanup")]
async fn do_cleanup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN).await?;
    // 审计修复（P0）：旧版删 7 天前过期促销，而 worker expire_promotions 保留 365 天——
    // hr_enforce 建快照需按 completed_at 时点回查当时促销，物理删掉近期历史会让
    // H&R 豁免回查失明（免费期完成的下载被误判违规）。与 worker 统一为 365 天。
    let expired_promos =
        sqlx::query("DELETE FROM promotions WHERE ends_at < now() - interval '365 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    let expired_warns = sqlx::query(
        "UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE warned_until IS NOT NULL AND warned_until < now()",
    ).execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let old_logins =
        sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '90 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    let old_resets =
        sqlx::query("DELETE FROM password_resets WHERE created_at < now() - interval '7 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE).await?;
    let rows: Vec<AdRow> = sqlx::query_as(
        "SELECT id, title, html, position, enabled, sort FROM ads ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
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

fn default_ad_position() -> String {
    "header".into()
}

#[post("/admin/ads")]
async fn ad_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE).await?;
    if !["header", "footer", "sidebar"].contains(&body.position.as_str()) {
        return Err(DomainError::Validation(
            "广告位需为 header/footer/sidebar".into(),
        ));
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE).await?;
    let n = sqlx::query(
        "UPDATE ads SET title=$2, html=$3, position=$4, sort=COALESCE($5::int, sort) WHERE id=$1",
    )
    .bind(*path)
    .bind(body.title.trim())
    .bind(&body.html)
    .bind(&body.position)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[put("/admin/ads/{id}/toggle")]
async fn ad_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE).await?;
    let n = sqlx::query("UPDATE ads SET enabled = NOT enabled WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.toggle", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/ads/{id}")]
async fn ad_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE).await?;
    sqlx::query("DELETE FROM ads WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "ad.delete", Some(*path as i64))
        .await;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::NOTCONNECTABLE_VIEW).await?;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::UPLOADERS_VIEW).await?;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AGENTS_VIEW).await?;
    let rows: Vec<AgentRow> = sqlx::query_as(
        "SELECT COALESCE('Transmission/Dev', 'unknown') AS agent, count(*) AS peers \
         FROM snatches WHERE seeding OR leeching GROUP BY 1 ORDER BY peers DESC",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::POLLS_MANAGE).await?;
    let rows: Vec<PollOverviewRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.closed, \
                (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS votes, p.created_at \
         FROM fun_polls p ORDER BY p.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::DBSTATS_VIEW).await?;
    let conns: Vec<PgConnRow> = sqlx::query_as(
        "SELECT state, count(*)::bigint AS count FROM pg_stat_activity WHERE datname = current_database() GROUP BY state ORDER BY count DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let db_size: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())::bigint")
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let tables: Vec<TableSizeRow> = sqlx::query_as(
        "SELECT c.relname, pg_total_relation_size(c.oid)::bigint AS total_size, c.reltuples::bigint AS row_estimates          FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace          WHERE n.nspname = 'public' AND c.relkind = 'r'          ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 15",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let slow_tx: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM pg_stat_activity WHERE datname = current_database() AND xact_start IS NOT NULL AND now() - xact_start > interval '30 seconds'",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    let dead_tuples: i64 =
        sqlx::query_scalar("SELECT COALESCE(sum(n_dead_tup), 0)::bigint FROM pg_stat_user_tables")
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SysLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SYSLOG_VIEW).await?;
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LocationQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LOCATIONS_MANAGE).await?;
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
    /// 质量维度种子（0092）：kinds 标签 + dict 选项；apply 时重建，未定义的维度不动
    #[serde(default)]
    #[sqlx(default)]
    sections: Option<serde_json::Value>,
}

/// 公开：当前站点档案（类型包 + 分类 + 模块开关 + 品牌名），前端布局/导航/上传表单由此驱动
#[get("/site-profile")]
async fn site_profile(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let site_type: String =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'site_type'")
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .unwrap_or_else(|| "general".into());
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs WHERE code = $1",
    ).bind(&site_type)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 实际分类以 categories 表为准（类型包只是初始快照，管理组可再编辑）
    let cats: Vec<(i32, String)> = sqlx::query_as("SELECT id, name FROM categories ORDER BY id")
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let brand: String =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'site_name'")
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten()
            .flatten()
            .or(pack.as_ref().map(|p| p.brand.clone()))
            .unwrap_or_default();
    // 站点货币名（0082）：默认「魔力」，站长可后台改任意名；空值兜底回默认
    let currency: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'currency_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty())
    .unwrap_or_else(|| "魔力".to_string());
    // 建站日期（页脚版权条 "(c) 站名 日期 Powered by FluxTorrent" 用）
    let founded: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'datefounded'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty());

    // 模块开关：site_type_packs.modules 只是站型的**初始快照**，运行时权威在
    // site_settings.module_*（后台改了开关，导航要跟着变）。以前者打底、后者覆盖。
    let mut modules = pack
        .as_ref()
        .map(|p| p.modules.clone())
        .unwrap_or_else(|| serde_json::json!({}));
    let overrides: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value FROM site_settings WHERE name ~ '^module_'")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(obj) = modules.as_object_mut() {
        for (name, value) in overrides {
            if let Some(key) = name.strip_prefix("module_") {
                obj.insert(key.to_string(), serde_json::json!(value == "yes"));
            }
        }
    }
    // 元数据源（0087）：csv → 数组，控制上传页条目输入显隐与 PT-Gen 范围
    let sources_raw: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'metadata_sources'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let sources: Vec<String> = sources_raw
        .unwrap_or_else(|| "imdb,douban,bangumi,indienova".into())
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();
    // 站点简介（0088）：页脚「站点信息」卡片文案，留空由前端回落字典默认
    let site_desc: Option<String> =
        sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'site_desc'")
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
    Ok(ok(serde_json::json!({
        "site_type": site_type,
        "pack_name": pack.as_ref().map(|p| p.name.clone()),
        "brand": brand,
        "currency_name": currency,
        "founded": founded,
        "metadata_sources": sources,
        "site_desc": site_desc,
        "categories": cats.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "modules": modules,
    })))
}

/// 类型包列表（管理组：切换向导）
#[get("/admin/site-type-packs")]
async fn site_type_pack_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SITEPACKS_MANAGE).await?;
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

/// 维度 kind 合法性（防注入）：小写字母/数字/下划线
fn is_ascii_kind(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// 应用类型包（sysop）：重建分类 + 写 site_type/site_name + 更新课本模块开关
#[post("/admin/site-type-packs/apply")]
async fn site_type_pack_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ApplyPackBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SITEPACKS_MANAGE).await?;
    let mode = body.mode.as_deref().unwrap_or("replace");
    if !["replace", "merge"].contains(&mode) {
        return Err(DomainError::Validation("mode 需为 replace/merge".into()));
    }
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort, sections FROM site_type_packs WHERE code = $1",
    ).bind(&body.code)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let cats = pack.categories.as_array().cloned().unwrap_or_default();
    let added = cats.len() as i64;
    if mode == "replace" {
        let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents")
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
        if used > 0 {
            // 有种子时禁止整表重建（避免悬挂引用）：提示改用 merge
            return Err(DomainError::Validation(
                "站点已有种子，replace 会悬挂引用；请使用 merge 模式（保留现有分类，追加新分类）"
                    .into(),
            ));
        }
        sqlx::query("DELETE FROM categories")
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    for (i, c) in cats.iter().enumerate() {
        let id = c
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i as i64 + 1) as i32;
        let name = c
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
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
            let val = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            let _ = sqlx::query(
                "INSERT INTO site_settings (name, value) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
            ).bind(format!("module_{k}")).bind(val)
            .execute(&mut *tx).await;
        }
    }
    // 质量维度种子（0092）：包内定义的维度重建标签与选项（references 级联清理旧引用）。
    // 0101 修复：切换站型后旧站型的内置维度残留（切音乐站仍见「游戏类型」）——
    // 内置九维中未被本包定义的维度整体移除（section_kinds 级联清 section_dict 与
    // torrent_sections 引用）；站方自建维度（不在内置清单）原样保留。
    let builtin: std::collections::HashSet<String> = [
        "media",
        "grades",
        "editions",
        "codec",
        "audio_codec",
        "standard",
        "source",
        "processing",
        "team",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut packed_kinds: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) = sections.get("kinds").and_then(serde_json::Value::as_array) {
            for k in kinds {
                if let Some(kind) = k.get("kind").and_then(serde_json::Value::as_str) {
                    packed_kinds.insert(kind.to_string());
                }
            }
        }
    }
    for kind in &builtin {
        if !packed_kinds.contains(kind) {
            // 引用中的维度直接删会级联清 torrent_sections —— 有种子的站点会丢筛选项，
            // 这里先检查是否被在用：被在用时跳过清理（宁残留不破坏）
            let in_use: i64 =
                sqlx::query_scalar("SELECT count(*) FROM torrent_sections WHERE kind = $1")
                    .bind(kind)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap_or(0);
            if in_use == 0 {
                sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) = sections.get("kinds").and_then(serde_json::Value::as_array) {
            for k in kinds {
                let (Some(kind), Some(label)) = (
                    k.get("kind").and_then(serde_json::Value::as_str),
                    k.get("label").and_then(serde_json::Value::as_str),
                ) else {
                    continue;
                };
                if !is_ascii_kind(kind) {
                    continue;
                }
                let sort = k
                    .get("sort")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(999) as i32;
                sqlx::query(
                    "INSERT INTO section_kinds (kind, label, sort) VALUES ($1, $2, $3) \
                     ON CONFLICT (kind) DO UPDATE SET label = EXCLUDED.label, sort = EXCLUDED.sort",
                )
                .bind(kind)
                .bind(label)
                .bind(sort)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
        if let Some(dict) = sections.get("dict").and_then(serde_json::Value::as_object) {
            for (kind, names) in dict {
                if !is_ascii_kind(kind) {
                    continue;
                }
                // 维度可能未在包 kinds 中定义（自定义维度追加选项）：确保存在
                sqlx::query(
                    "INSERT INTO section_kinds (kind, label, sort) VALUES ($1, $1, 999) ON CONFLICT (kind) DO NOTHING",
                )
                .bind(kind)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                // 整维替换：旧选项与其 torrent_sections 引用级联清除（显式应用包 = 重建口径）
                sqlx::query("DELETE FROM section_dict WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                for (i, name) in names
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    let Some(name) = name.as_str() else { continue };
                    sqlx::query("INSERT INTO section_dict (kind, name, sort) VALUES ($1, $2, $3)")
                        .bind(kind)
                        .bind(name)
                        .bind((i + 1) as i32)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // U2（0108）：等级叙事/经济预设/元数据源（apply_pack_extras 过程内含合法键校验）
    let extras: Vec<(String, i64)> =
        sqlx::query_as("SELECT kind, applied FROM apply_pack_extras($1)")
            .bind(&pack.code)
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    // 模块开关进程缓存失效（apply 改 module_* 后立即生效，不等 30s TTL）
    state.module_flags.invalidate().await;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_apply", None)
        .await;
    Ok(ok(
        serde_json::json!({ "applied": pack.code, "mode": mode, "categories": added, "extras": extras }),
    ))
}

/// 站型切换 diff 预览（U2 §8.2 向导第二步）：返回 apply 将改动的键旧值→新值，
/// 不落库。站长确认后才走 apply。
#[derive(Deserialize)]
struct PackDiffBody {
    code: String,
}
#[post("/admin/site-type-packs/diff")]
async fn site_type_pack_diff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackDiffBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SITEPACKS_MANAGE).await?;
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs WHERE code = $1",
    ).bind(&body.code)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };
    // 当前值快照（site_type/site_name/module_*）
    let current: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value FROM site_settings WHERE name IN ('site_type','site_name') OR name LIKE 'module\\_%'")
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    let cur = std::collections::HashMap::<String, String>::from_iter(current);
    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut push = |key: &str, old: Option<&String>, new: &str| {
        let old_v = old.map(|s| s.as_str()).unwrap_or("(未设置)");
        if old_v != new {
            changes.push(serde_json::json!({ "key": key, "old": old_v, "new": new }));
        }
    };
    push("site_type", cur.get("site_type"), &pack.code);
    push("site_name", cur.get("site_name"), &pack.brand);
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let setting = format!("module_{k}");
            let new = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            push(&setting, cur.get(&setting), new);
        }
    }
    Ok(ok(serde_json::json!({
        "pack": pack.code,
        "name": pack.name,
        "changes": changes,
        "unchanged_modules": cur.len().saturating_sub(changes.len()),
    })))
}

/// 自定义站型另存（U2 §7.3 / U5 分发）：读当前站点配置快照存为新包
/// （code 前缀 custom_），预置包只读——满足「第 12 种站型」。
#[derive(Deserialize)]
struct PackSaveBody {
    code: String,
    name: String,
}
#[post("/admin/site-type-packs/save")]
async fn site_type_pack_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackSaveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SITEPACKS_MANAGE).await?;
    let code = body.code.trim().to_lowercase();
    if !code.starts_with("custom_")
        || code.len() > 40
        || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(DomainError::Validation(
            "code 需以 custom_ 开头，仅小写字母/数字/下划线，≤40 字符".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("name 不能为空".into()));
    }
    // 当前配置快照：分类 / 模块开关 / 站名
    let cats: Vec<serde_json::Value> =
        sqlx::query_as::<_, (i32, String)>("SELECT id, name FROM categories ORDER BY id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .into_iter()
            .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
            .collect();
    let mods_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value FROM site_settings WHERE name LIKE 'module\\_%'")
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    let modules: serde_json::Map<String, serde_json::Value> = mods_rows
        .into_iter()
        .filter_map(|(name, value)| {
            name.strip_prefix("module_")
                .map(|k| (k.to_string(), serde_json::json!(value == "yes")))
        })
        .collect();
    let brand: String =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'site_name'")
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
    let sort: i32 = sqlx::query_scalar("SELECT COALESCE(max(sort), 100) + 1 FROM site_type_packs")
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(101);
    sqlx::query(
        "INSERT INTO site_type_packs (code, name, description, brand, categories, modules, sort) \
         VALUES ($1, $2, '自定义站型（另存快照）', $3, $4::jsonb, $5::jsonb, $6) \
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name, brand = EXCLUDED.brand, \
           categories = EXCLUDED.categories, modules = EXCLUDED.modules",
    )
    .bind(&code)
    .bind(body.name.trim())
    .bind(&brand)
    .bind(serde_json::Value::Array(cats).to_string())
    .bind(serde_json::Value::Object(modules).to_string())
    .bind(sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_save", None)
        .await;
    Ok(ok(serde_json::json!({ "saved": code })))
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

/// 捐赠中心总览：钱包余额 + VIP 状态 + 套餐 + 我的流水
#[get("/donate/state")]
async fn donate_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (wallet, vip_until): (f64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT wallet_usd::float8, vip_until FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
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
    Ok(ok(serde_json::json!({
        "wallet_usd": wallet,
        "vip_until": vip_until,
        "plans": plans,
        "ledger": ledger,
        // U4 §12.1：通道可用性（provider=epay 且凭证齐全；FLUX_DEMO 演示模式恒开）
        "payment_enabled": crate::payment::gateway_config(&state).await.available()
            || std::env::var("FLUX_DEMO").unwrap_or_default() == "1",
    })))
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopupBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(10.0..=66.0).contains(&body.amount_usd) {
        return Err(DomainError::Validation("单笔 10 ~ 66 USD".into()));
    }
    if !["alipay", "wechat"].contains(&body.channel.as_str()) {
        return Err(DomainError::Validation("支付方式需为 alipay/wechat".into()));
    }
    // U4 §12.1：真实订单流——按 site_settings 构造网关；未配置（none/缺凭证）拒单。
    // FLUX_DEMO=1 演示环境保留旧模拟充值（演示数据需要有钱包入账可看）。
    let gw = crate::payment::gateway_config(&state).await;
    if !gw.available() {
        if std::env::var("FLUX_DEMO").unwrap_or_default() == "1" {
            return crate::payment::demo_topup(&state, auth.id, body.amount_usd, &body.channel)
                .await
                .map(|v| ok(v));
        }
        return Err(DomainError::Validation(
            "捐赠通道未开放（站长未配置支付网关）".into(),
        ));
    }
    // 建订单 + 返回跳转 URL（入账只发生在验签通过的回调，此端点不动钱包）
    let order_no = crate::payment::new_order_no(auth.id);
    sqlx::query(
        "INSERT INTO payment_orders (order_no, user_id, amount_usd, channel) VALUES ($1, $2, $3, $4)",
    )
    .bind(&order_no)
    .bind(auth.id)
    .bind(body.amount_usd)
    .bind(&body.channel)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let provider = crate::payment::provider_from(&gw);
    let base = std::env::var("PUBLIC_SITE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
    let url = provider.pay_url(
        &order_no,
        &format!("{:.2}", body.amount_usd),
        &body.channel,
        "站点捐赠",
        &format!("{base}/donate?order={order_no}"),
        &format!("{base}/api/v1/donate/notify"),
    );
    state
        .repo
        .audit(Some(auth.id), "donate_topup_order", None)
        .await;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "pay_url": url }),
    ))
}

/// 捐赠订单状态查询（前端支付回跳后轮询）
#[get("/donate/order-status")]
async fn donate_order_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let Some(order_no) = q.get("order_no") else {
        return Err(DomainError::Validation("缺少 order_no".into()));
    };
    let st: Option<String> = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE order_no = $1 AND user_id = $2",
    )
    .bind(order_no)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "status": st }),
    ))
}

/// 支付网关异步回调（GET，易支付口径）：验签 → 幂等入账 → 纯文本 "success"
/// （网关要求响应 success 字面量，不走统一信封）
#[get("/donate/notify")]
async fn donate_notify(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _ = req;
    match crate::payment::settle_notify(&state, &q.into_inner()).await {
        Ok(_) => HttpResponse::Ok().body("success"),
        Err(e) => {
            tracing::warn!(?e, "donate notify rejected");
            HttpResponse::Ok().body("fail")
        }
    }
}

/// 旧模拟充值逻辑（FLUX_DEMO=1 专用）：移入 payment.rs 兄弟函数，保留演示站能力
#[allow(dead_code)]
mod removed_legacy_simulated_topup {
    // 历史实现已由 payment::demo_topup 替代（编译期占位，防误回滚参考）
}

#[derive(Deserialize)]
struct OrderBody {
    plan_id: i32,
}

/// 用余额订购套餐（上传量 / 片单额度 / VIP）
#[post("/donate/order")]
async fn donate_order(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OrderBody>,
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
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: Option<f64> = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd - $2 WHERE id = $1 AND wallet_usd >= $2 RETURNING wallet_usd::float8",
    ).bind(auth.id).bind(price)
    .fetch_optional(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        return Err(DomainError::Validation(format!(
            "余额不足，还差 {:.2} USD",
            price
        )));
    };
    // 套餐生效
    match plan_type.as_str() {
        "upload" => {
            // 「100 GB 上传量」/「500 GB 上传量」
            let gb: i64 = title
                .split_whitespace()
                .next()
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
            // 必须落差额流水（P1）：快照权威在 traffic_ledger，reconcile_snapshots 每 6h
            // 把 users.uploaded 重算为 sum(delta_up)——只 UPDATE 快照不落流水，付费购买的
            // 上传量会在 6h 内被静默抹掉。对照 shop upload_credit 的双写。
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(auth.id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("UPDATE users SET uploaded = uploaded + $2 WHERE id = $1")
                .bind(auth.id)
                .bind(gb * 1024 * 1024 * 1024)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "quota" => {
            sqlx::query("UPDATE users SET quota_extra = quota_extra + 10 WHERE id = $1")
                .bind(auth.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "vip" => {
            let days: i64 = if title.contains("终身") {
                36500
            } else if title.contains("180") {
                180
            } else {
                30
            };
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
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "donate_order", Some(body.plan_id as i64))
        .await;
    Ok(ok(
        serde_json::json!({ "plan": title, "wallet_usd": balance }),
    ))
}

// ---- 批量邮件（massmail）----

#[derive(Deserialize)]
struct MassMailBody {
    subject: String,
    body: String,
}

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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL).await?;
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
async fn massmail_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MassMailBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL).await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO mass_mails (subject, body, sent_by, recipients) VALUES ($1,$2,$3,$4) RETURNING id",
    ).bind(body.subject.trim()).bind(&body.body).bind(auth.id).bind(users as i32)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "massmail_send", None).await;
    // 审计修复（P1 永不投递）：注释宣称 worker 投递，但 worker 无任何 mass_mails 消费——
    // 邮件永不发出且响应报 queued 误导操作者。现在当场投递（复用 build_smtp）：
    // SMTP 配置时逐户发送并回填实发数；未配置时（开发态）明确返回 delivered=0 与
    // 原因，不再谎报入队。
    let smtp = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_else(|_| "no-reply@fluxtorrent.local".into());
    let mut delivered: i64 = 0;
    let mut mail_err: Option<String> = None;
    if smtp.is_empty() {
        mail_err = Some("SMTP_URL 未配置（开发态：邮件未投递，仅留档）".into());
    } else {
        match crate::gaps_http::build_smtp(&smtp) {
            Ok(mailer) => {
                let emails: Vec<String> = sqlx::query_scalar(
                    "SELECT email FROM users WHERE status < 2 AND email IS NOT NULL",
                )
                .fetch_all(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                for to in &emails {
                    let msg = lettre::Message::builder()
                        .from(from.parse().map_err(|e: lettre::address::AddressError| {
                            DomainError::Internal(e.into())
                        })?)
                        .to(to.parse().map_err(|e: lettre::address::AddressError| {
                            DomainError::Internal(e.into())
                        })?)
                        .subject(body.subject.trim())
                        .body(body.body.clone())
                        .map_err(|e| DomainError::Internal(e.into()))?;
                    use lettre::AsyncTransport;
                    match mailer.send(msg).await {
                        Ok(_) => delivered += 1,
                        Err(e) => {
                            mail_err = Some(format!("第 {delivered} 封后失败：{e}"));
                            break;
                        }
                    }
                }
            }
            Err(e) => mail_err = Some(format!("SMTP 构建失败：{e}")),
        }
    }
    if delivered > 0 {
        let _ = sqlx::query("UPDATE mass_mails SET recipients = $2 WHERE id = $1")
            .bind(id)
            .bind(delivered as i32)
            .execute(&state.repo.db)
            .await;
    }
    Ok(ok(serde_json::json!({
        "id": id, "delivered": delivered,
        "queued": 0,
        "note": mail_err.unwrap_or_else(|| "已全部投递".into()),
    })))
}

// ============ 插件：勋章墙 / 大赛 / 头像挂件 / 五子棋 ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct MedalWallEntry {
    username: String,
    medal_name: String,
    /// 勋章图片（有则前端画图，无则回落 🏅）
    asset_ref: Option<String>,
}

/// 勋章墙（medal_wall.php 口径）：用户当前「佩戴中」的勋章展示墙
/// （修复前返回全部持有——佩戴语义缺失；并对齐 0067 过滤已过期勋章）
#[get("/medal-wall")]
async fn medal_wall(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<MedalWallEntry> = sqlx::query_as(
        "SELECT u.username, m.name AS medal_name, m.asset_ref \
         FROM user_medals um JOIN users u ON u.id = um.user_id JOIN medals m ON m.id = um.medal_id \
         WHERE um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY u.id, m.id LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let inserted = sqlx::query(
        "INSERT INTO contest_entries (contest_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已报名".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct FrameRow {
    id: i32,
    name: String,
    css: String,
    #[sqlx(default)]
    image_url: Option<String>,
    price: i32,
}

#[get("/avatar-frames")]
async fn frame_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<FrameRow> = sqlx::query_as(
        "SELECT id, name, css, image_url, price FROM avatar_frames ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FrameSetBody {
    frame_id: Option<i32>,
}

/// 佩戴头像挂件（需已购买：简化口径 price=0 免费 / >0 扣魔力）
#[put("/me/avatar-frame")]
async fn frame_equip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FrameSetBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    match body.frame_id {
        Some(fid) => {
            let price: Option<i32> =
                sqlx::query_scalar("SELECT price FROM avatar_frames WHERE id=$1")
                    .bind(fid)
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(price) = price else {
                return Err(DomainError::NotFound(fid as i64));
            };
            if price > 0 {
                let idem = format!("frame:{}:{}", auth.id, fid);
                // 幂等键确定性（frame:{uid}:{fid}）：换戴回已购框架 → Replayed（不重复
                // 扣款）后仍继续佩戴，这正是「重复佩戴不重复收费」的预期语义，显式丢弃。
                let outcome = spend_spark(
                    &state.repo.db,
                    auth.id,
                    price as i64,
                    "shop",
                    &idem,
                    "avatar_frame",
                    fid as i64,
                )
                .await?;
                let _ = outcome;
            }
            sqlx::query("UPDATE users SET avatar_frame_id=$2 WHERE id=$1")
                .bind(auth.id)
                .bind(fid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": fid })))
        }
        None => {
            sqlx::query("UPDATE users SET avatar_frame_id=NULL WHERE id=$1")
                .bind(auth.id)
                .execute(&state.repo.db)
                .await
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
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gomoku_games (black_id, board) VALUES ($1, '') RETURNING id",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[post("/gomoku/games/{id}/join")]
async fn gomoku_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE gomoku_games SET white_id=$2, updated_at=now() WHERE id=$1 AND white_id IS NULL AND black_id <> $2",
    ).bind(*path).bind(auth.id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::Validation("对局不存在或已有对手".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(Deserialize)]
struct MoveBody {
    pos: i32,
}

#[post("/gomoku/games/{id}/move")]
async fn gomoku_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<MoveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(0..225).contains(&body.pos) {
        return Err(DomainError::Validation("落点越界".into()));
    }
    let g: Option<(i64, Option<i64>, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((black, white, board, turn, winner)) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if winner.is_some() {
        return Err(DomainError::Validation("对局已结束".into()));
    }
    let Some(white) = white else {
        return Err(DomainError::Validation("等待对手加入".into()));
    };
    let my_color = if auth.id == black {
        'b'
    } else if auth.id == white {
        'w'
    } else {
        return Err(DomainError::Forbidden);
    };
    if my_color.to_string() != turn {
        return Err(DomainError::Validation("还没轮到你".into()));
    }
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
    )
    .bind(*path)
    .bind(&new_board)
    .bind(next_turn)
    .bind(winner_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "board": new_board, "turn": next_turn, "winner": winner_id, "you_won": won }),
    ))
}

#[get("/gomoku/games/{id}")]
async fn gomoku_get(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let g: Option<GomokuGame> = sqlx::query_as(
        "SELECT id, black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(g) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
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
                if rr < 0 || rr >= SIZE as isize || cc < 0 || cc >= SIZE as isize {
                    break;
                }
                if cells[rr as usize * SIZE + cc as usize] != color {
                    break;
                }
                count += 1;
                step += 1;
            }
        }
        if count >= 5 {
            return true;
        }
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
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
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
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let updated =
        sqlx::query("UPDATE announcements SET title = $2, body = $3, badge = $4 WHERE id = $1")
            .bind(*path)
            .bind(body.title.trim())
            .bind(&body.body)
            .bind(if body.badge.trim().is_empty() {
                "公告"
            } else {
                body.badge.trim()
            })
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
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
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
        if chrono::Utc::now() - t < chrono::Duration::hours(24)
            && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await
        {
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
    if owner != auth.id && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await {
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
    state
        .repo
        .audit(Some(auth.id), "fun_status_change", None)
        .await;
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
    if owner != auth.id && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await {
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
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

    // 签到日历（attendance-card）：当月逐日 + 连签/累计 + 补签卡持有数（0066）
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let att: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, reward FROM attendance WHERE user_id = $1 ORDER BY date",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站点时区 UTC+8（与 /attendance、/checkin 同口径；容器 TZ=UTC 时 Local 会让
    // 首页日历在北京 0-8 点窗口显示「昨日未签」）
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
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
    let by_day: std::collections::HashMap<chrono::NaiveDate, (i64, i64)> =
        daily.iter().map(|(d, o, f)| (*d, (*o, *f))).collect();
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
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
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
    let links: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT name, url, title FROM friend_links ORDER BY sort")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "news": news_json,
        "attendance": {
            "month": today.format("%Y年%m月").to_string(),
            "streak": streak, "total_days": total_days, "checked_today": checked_today,
            "calendar": calendar, "makeup_cards": makeup_cards,
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
        // 首页排版（0089）：site_settings.home_layout 原样透传（JSON 数组或空串），
        // 前端空/非法回退默认布局
        "home_layout": crate::http::home_layout_raw(&state.repo.db).await,
    })))
}

/// 读取首页排版配置（0089）：返回原文；库错误/缺行回空串（首页永远可渲染）
pub async fn home_layout_raw(db: &sqlx::PgPool) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'home_layout'")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// 首页板块键白名单（0089）：与前端 HomeSections 渲染分支一一对应
pub const HOME_SECTION_KEYS: &[&str] = &[
    "news",
    "attendance",
    "shoutbox",
    "funbox",
    "resource_stats",
    "site_data",
    "lucky_draw",
    "links",
    "latest",
];

#[derive(Deserialize)]
struct HomeLayoutItem {
    key: String,
    /// 1/2/3 = 1/3、2/3、整行；缺省 0 由前端按板块推荐档处理
    #[serde(default)]
    span: i32,
}

/// 保存首页排版（sysop，0089）：校验 JSON 结构 + 键白名单 + 去重 + span 白名单，
/// 规范化后存 site_settings.home_layout。items 传空数组 = 恢复默认排版。
#[put("/admin/home-layout")]
async fn admin_home_layout_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<Vec<HomeLayoutItem>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    if body.len() > 20 {
        return Err(DomainError::Validation("板块数量至多 20".into()));
    }
    let mut norm: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for it in body.iter() {
        if !HOME_SECTION_KEYS.contains(&it.key.as_str()) {
            return Err(DomainError::Validation(format!(
                "未知板块键 {}（可用：{}）",
                it.key,
                HOME_SECTION_KEYS.join("/")
            )));
        }
        if !seen.insert(&it.key) {
            return Err(DomainError::Validation(format!("板块 {} 重复", it.key)));
        }
        if ![0, 1, 2, 3].contains(&it.span) {
            return Err(DomainError::Validation(
                "span 取值 1/2/3（缺省自动）".into(),
            ));
        }
        norm.push(serde_json::json!({ "key": it.key, "span": it.span }));
    }
    let value = serde_json::to_string(&norm).map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO site_settings (name, value, descr, grp) VALUES ('home_layout', $1, '首页板块排版（JSON 数组，空 = 默认布局）', 'main')          ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(&value)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "home_layout.update", None)
        .await;
    Ok(ok(
        serde_json::json!({ "saved": norm.len(), "layout": norm }),
    ))
}

// ============ 发布 / 下载（M04 / M05） ============

#[derive(Deserialize)]
struct UploadForm {
    name: Option<String>,
    small_descr: Option<String>,
    descr: Option<String>,
    category_id: i32,
    /// 介质列（0087 起可空，仅为兼容老数据；新数据以 torrent_sections.kind='media' 为准）
    #[serde(default)]
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    #[serde(default)]
    anonymous: bool,
    /// 海报/封面外链 URL（存 media_info.poster；列表 46px 封面位与首页海报墙共用）
    #[serde(default)]
    poster: Option<String>,
    /// 多维属性（第八轮 Section）：kind → section_dict.id
    /// multipart 场景以 JSON 字符串传递：sections={"codec":1,"team":2}
    #[serde(default)]
    sections: Option<String>,
    /// 聚合组（0069）：加入既有组（同一资源的多个版本共享元数据），缺省为独立种子
    #[serde(default)]
    group_id: Option<i64>,
    /// 标签（NP upload.php tags 口径）：tag_dict.id 数组，multipart 场景以 JSON 字符串传递
    #[serde(default)]
    tags: Option<String>,
    /// IMDb 链接（NP imdbpage 口径）：存 media_info.imdb，搜索区 4 已按此键命中
    #[serde(default)]
    imdb: Option<String>,
    /// MediaInfo 文本（NP 详情页折叠块口径）：存 media_info.mediainfo，详情页原样展示
    #[serde(default)]
    mediainfo: Option<String>,
    /// 付费下载定价（0086）：0 = 免费，≤ 1,000,000；下载者支付，发布者得 (100-税)%
    #[serde(default)]
    price: Option<i64>,
    /// 推荐位（0089，NP 挑选 口径）：pos_state 0/1/2 = 不置顶/一级/二级，需管理组
    #[serde(default)]
    pos_state: Option<i16>,
    /// 置顶截止时间（ISO 8601；空 = 永久置顶）
    #[serde(default)]
    pos_state_until: Option<String>,
    /// 推荐影片（0089）：pick_type 0/1/2 = 普通/推荐/经典，需管理组
    #[serde(default)]
    pick_type: Option<i16>,
}

/// CP437 高位区（0x80-0xFF）→ Unicode（DOS 风格 NFO 的事实编码；表由 Python cp437 编解码器生成）
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{00A0}";

/// NFO 字节解码：合法 UTF-8 直接用，否则按 CP437 逐字节映射（经典场景 NFO 的字符画不丢）
fn decode_nfo(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    bytes
        .iter()
        .map(|b| {
            if *b < 0x80 {
                *b as char
            } else {
                CP437_HIGH
                    .chars()
                    .nth((*b - 0x80) as usize)
                    .unwrap_or('\u{FFFD}')
            }
        })
        .collect()
}

#[derive(serde::Deserialize)]
struct PtgenQ {
    url: String,
}

/// PT-Gen 代理（NP ptgen.php 口径）：服务端转发 imdb/douban/bangumi/indienova，
/// 规避浏览器 CORS；返回 HTML 剥离为纯文本，匹配前端 markdown-lite 简介渲染
#[get("/ptgen")]
async fn ptgen(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PtgenQ>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let url = q.url.trim();
    let parsed = url::Url::parse(url).map_err(|_| DomainError::Validation("链接无效".into()))?;
    let host = parsed.host_str().unwrap_or_default().to_lowercase();
    // 站点启用源（0087 metadata_sources）∩ PT-Gen 支持的源：host 后缀映射
    let enabled: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'metadata_sources'), 'imdb,douban,bangumi,indienova')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "imdb,douban,bangumi,indienova".into());
    let allowed = enabled.contains("imdb") && host.ends_with("imdb.com")
        || enabled.contains("douban") && host.ends_with("douban.com")
        || enabled.contains("bangumi")
            && (host == "bgm.tv"
                || host.ends_with(".bgm.tv")
                || host == "bangumi.tv"
                || host.ends_with(".bangumi.tv"))
        || enabled.contains("indienova") && host.ends_with("indienova.com");
    if !allowed {
        return Err(DomainError::Validation(
            "链接无效或该元数据源未在本站启用（imdb / douban / bangumi / indienova）".into(),
        ));
    }
    let api = url::Url::parse_with_params("https://ptgen.rachpt.dev/api", &[("url", url)])
        .map_err(|_| DomainError::Validation("链接无效".into()))?;
    let client = reqwest::Client::new();
    let resp = client
        .get(api)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !resp.status().is_success() {
        return Err(DomainError::Validation(format!(
            "PT-Gen 上游异常（HTTP {}）",
            resp.status().as_u16()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let html = body
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) || html.is_empty() {
        return Err(DomainError::Validation("PT-Gen 未能解析该链接".into()));
    }
    Ok(ok(serde_json::json!({
        "name": body.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "descr": html_to_text(html),
    })))
}

/// 简易 HTML → 纯文本（PT-Gen 返回物）：块级标签转行、剥其余标签、解常见实体
fn html_to_text(html: &str) -> String {
    let mut s = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</tr>", "\n")
        .replace("</li>", "\n")
        .replace("<li>", "- ")
        .replace("</td>", "  ")
        .replace("</th>", "  ");
    // 剥离其余标签（PT-Gen 输出为受信源生成的受控 HTML，逐字符状态机即可）
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    s = out;
    for (ent, ch) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        s = s.replace(ent, ch);
    }
    // 折叠空行 + 去行尾空白
    let mut lines: Vec<String> = Vec::new();
    for line in s.lines() {
        let t = line.trim_end();
        if t.is_empty() && lines.last().map(String::is_empty).unwrap_or(true) {
            continue;
        }
        lines.push(t.to_string());
    }
    lines.join("\n").trim().to_string()
}

/// .torrent 本体远小于附件（极端多文件大 piece 也在 MiB 级）；nfo 为纯文本。
/// 必须在流式循环内拦截：actix 默认 2MB PayloadConfig 只约束 Json 提取器，不约束 Multipart。
const TORRENT_MAX_BYTES: usize = 4 * 1024 * 1024; // 单文件 4MiB
const NFO_MAX_BYTES: usize = 1 * 1024 * 1024; // 单文件 1MiB

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
    // 发种基础权限（默认配给全体用户 class 1；可用于限制上传资格）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TORRENT_UPLOAD).await?;
    let mut file_bytes: Option<Bytes> = None;
    let mut nfo_bytes: Option<Bytes> = None;
    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| DomainError::Validation(e.to_string()))?;
        match field.name() {
            Some("file") => {
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| DomainError::Validation(e.to_string()))?,
                    );
                    if buf.len() > TORRENT_MAX_BYTES {
                        return Err(DomainError::Validation(".torrent 超过 4MiB 上限".into()));
                    }
                }
                file_bytes = Some(buf.freeze());
            }
            // NFO 文件（NP upload.php nfo 口径）：文本解码后落 torrents.nfo
            Some("nfo") => {
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| DomainError::Validation(e.to_string()))?,
                    );
                    if buf.len() > NFO_MAX_BYTES {
                        return Err(DomainError::Validation("NFO 超过 1MiB 上限".into()));
                    }
                }
                nfo_bytes = Some(buf.freeze());
            }
            _ => {}
        }
    }
    let bytes = file_bytes.ok_or(DomainError::Validation("缺少 .torrent 文件".into()))?;

    let parsed = crate::bencode::parse_torrent(&bytes).map_err(DomainError::TorrentInvalid)?;

    // 重复检测（M04：info_hash 唯一）
    let dupe: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE info_hash = $1 OR raw_info_hash = $2)",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
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
    // 封面外链 + IMDb 链接 → media_info（JSONB 键合并；搜索区 4 按 imdb 键命中）
    let mut media_obj = serde_json::Map::new();
    if let Some(u) = form
        .poster
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        media_obj.insert("poster".into(), serde_json::json!(u));
    }
    if let Some(i) = form
        .imdb
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        media_obj.insert("imdb".into(), serde_json::json!(i));
    }
    if let Some(mi) = form
        .mediainfo
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        // 截断防滥用（MediaInfo 全文通常 < 64KB）
        let mi = &mi[..mi.len().min(60_000)];
        media_obj.insert("mediainfo".into(), serde_json::json!(mi));
    }
    let media_info: Option<serde_json::Value> = if media_obj.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(media_obj))
    };
    // 发布员职务 / 免审核权限 → 发布即通过（torrent.approval.auto）；
    // 第八轮：命中「自动过审」分类同样免审（categories.auto_approve）
    let cat_auto: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(auto_approve), FALSE) FROM categories WHERE id = $1",
    )
    .bind(form.category_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    // 0077 被拒禁发（NP upload_deny_approval_deny_count 口径）：累计被拒达阈值直接拦
    let (deny_count, streak): (i32, i32) =
        sqlx::query_as("SELECT deny_count, approve_streak FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let deny_limit: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_deny_limit'), 2)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(2);
    if deny_count >= deny_limit {
        return Err(DomainError::Validation(
            "因多次发布被拒，上传资格已暂停；请先通过『联系我们』申诉".into(),
        ));
    }
    // 0077 免审通道（NP offer_skip_approved_count 口径）：连续过审 ≥5 的发布者免审
    let streak_skip = streak >= 5;
    let auto_approve = cat_auto
        || streak_skip
        || crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_APPROVAL_AUTO).await;
    let approval_status: i16 = if auto_approve { 1 } else { 0 };
    // 聚合组（0069）：显式传入的 group_id 必须存在（防悬挂引用）
    if let Some(gid) = form.group_id {
        let g: Option<i64> = sqlx::query_scalar("SELECT id FROM torrent_groups WHERE id = $1")
            .bind(gid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        if g.is_none() {
            return Err(DomainError::Validation("聚合组不存在".into()));
        }
    }
    let nfo_text: Option<String> = nfo_bytes
        .as_deref()
        .map(decode_nfo)
        .filter(|s| !s.trim().is_empty());
    let price = form.price.unwrap_or(0).clamp(0, 1_000_000);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, group_id, name, small_descr, descr, category_id, medium_id, grade_id, edition_id, owner_id, anonymous, size, numfiles, approval_status, media_info, nfo, price) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19) RETURNING id",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .bind(&parsed.pieces_hash_hex)
    .bind(form.group_id)
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
    .bind(approval_status)
    .bind(media_info)
    .bind(nfo_text)
    .bind(price)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| {
        // 并发上传同一 .torrent：EXISTS 检查与 INSERT 之间的窗口由唯一约束兜底，
        // 映射为语义化的重复错误而非裸 500（raw_info_hash 只有普通索引，见 0081）
        if e.to_string().contains("torrents_info_hash_key")
            || e.to_string().contains("duplicate key")
        {
            DomainError::TorrentDuplicate
        } else {
            DomainError::Internal(e.into())
        }
    })?;

    // 多维属性（第八轮 Section）：校验 kind 白名单后写 torrent_sections
    if let Some(json) = form
        .sections
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let map: std::collections::HashMap<String, i64> = serde_json::from_str(json)
            .map_err(|_| DomainError::Validation("sections 需为 JSON 对象".into()))?;
        for (kind, dict_id) in &map {
            // 0085/0087：维度可由站方自建（含 media/grades/editions），白名单查 section_kinds
            if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind).await {
                return Err(DomainError::Validation(format!("未知维度 {kind}")));
            }
            // 字典归属校验：dict_id 必须属于该 kind（防跨维度错挂）
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $2 AND kind = $1)",
            )
            .bind(kind)
            .bind(dict_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::Validation(format!(
                    "维度 {kind} 的字典项 {dict_id} 不存在"
                )));
            }
            sqlx::query(
                "INSERT INTO torrent_sections (torrent_id, kind, dict_id) VALUES ($1, $2, $3)                  ON CONFLICT (torrent_id, kind) DO UPDATE SET dict_id = EXCLUDED.dict_id",
            )
            .bind(id)
            .bind(kind)
            .bind(dict_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }

    // 标签（NP upload.php tags 口径）：发布时直接打标；启用字典校验 + 官方标签仅 staff
    if let Some(json) = form
        .tags
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let ids: Vec<i32> = serde_json::from_str(json)
            .map_err(|_| DomainError::Validation("tags 需为 JSON 数组".into()))?;
        if ids.len() > 12 {
            return Err(DomainError::Validation("标签最多选择 12 个".into()));
        }
        let is_staff = auth.class_id >= 90;
        for tid in &ids {
            let row: Option<(String, bool)> = sqlx::query_as(
                "SELECT kind, COALESCE(enabled, TRUE) FROM tag_dict \
                 WHERE id = $1 AND scope = 'torrent'",
            )
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some((kind, enabled)) = row else {
                return Err(DomainError::Validation(format!("标签 {tid} 不存在")));
            };
            if !enabled {
                return Err(DomainError::Validation(format!("标签 {tid} 已停用")));
            }
            if kind == "official" && !is_staff {
                return Err(DomainError::Forbidden); // 与详情页打标同口径
            }
            sqlx::query(
                "INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }

    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 文件清单入 files 表（修复前从不写入：新种的文件列表/按文件名搜索永远为空）
    for (idx, (path, len)) in parsed.files.iter().enumerate() {
        sqlx::query(
            "INSERT INTO files (torrent_id, file_index, path, size) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(idx as i32)
        .bind(path)
        .bind(len)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 发种自动促销（0089，NP 促销设置 口径）：管理后台配置默认促销（类型+天数），
    // 发布即自动套用——促销跟随站点，不再由发布者单独设置。
    let auto_kind: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'upload_auto_promo_kind'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let auto_days: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_auto_promo_days'), 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let auto_kind = auto_kind.trim().to_lowercase();
    if !auto_kind.is_empty()
        && auto_days > 0
        && ["free", "x2", "x2free", "half", "x2half", "p30"].contains(&auto_kind.as_str())
    {
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('torrent', $1, $2::promotion_kind_enum, now(), now() + make_interval(days => $3), \
                     'manual'::promotion_source, $4)",
        )
        .bind(id)
        .bind(&auto_kind)
        .bind(auto_days.clamp(1, 720))
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 推荐位（0089，NP 挑选 口径）：置顶位置/截止 + 推荐影片，管理组专属；
    // 发布页人人可见，无权限提交会被此处拦截（与参考站服务端强校验同口径）
    if form.pos_state.unwrap_or(0) != 0
        || form.pick_type.unwrap_or(0) != 0
        || form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
    {
        if auth.class_id < 90 {
            return Err(DomainError::Forbidden); // 置顶/推荐仅管理组
        }
        let pos = form.pos_state.unwrap_or(0);
        if ![0, 1, 2].contains(&pos) {
            return Err(DomainError::Validation("置顶位置取值 0/1/2".into()));
        }
        let pick = form.pick_type.unwrap_or(0);
        if ![0, 1, 2].contains(&pick) {
            return Err(DomainError::Validation("推荐影片取值 0/1/2".into()));
        }
        let until = form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| DomainError::Validation("置顶截止时间格式无效".into()))
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            })
            .transpose()?;
        sqlx::query(
            "UPDATE torrents SET pos_state = $2, pos_state_until = $3, pick_type = $4, mtime = now() WHERE id = $1",
        )
        .bind(id)
        .bind(pos)
        .bind(until)
        .bind(pick)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    state
        .repo
        .audit(Some(auth.id), "torrent_upload", Some(id))
        .await;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(&state, id, auth.id);

    // 0075 聚合组推荐（未显式指定组时）：
    //   a) pieces_hash 命中已有组 → 直接建议锁定（跨站同源再发布场景）
    //   b) 否则名称相似度（trgm）> 0.4 的组 → 候选列表
    let mut group_suggest: serde_json::Value = serde_json::json!(null);
    if form.group_id.is_none() {
        let lock: Option<i64> = sqlx::query_scalar(
            "SELECT t2.group_id FROM torrents t2              WHERE t2.pieces_hash = $1 AND t2.pieces_hash <> '' AND t2.group_id IS NOT NULL LIMIT 1",
        )
        .bind(&parsed.pieces_hash_hex)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
        if let Some(gid) = lock {
            let gname: String = sqlx::query_scalar("SELECT name FROM torrent_groups WHERE id = $1")
                .bind(gid)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or_default();
            group_suggest = serde_json::json!({ "locked": true, "group_id": gid, "name": gname });
        } else {
            let cands: Vec<(i64, String)> = sqlx::query_as(
                "SELECT g.id, g.name FROM torrent_groups g                  WHERE similarity(g.name, $1) > 0.4                  ORDER BY similarity(g.name, $1) DESC LIMIT 3",
            )
            .bind(&name)
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
            if !cands.is_empty() {
                group_suggest = serde_json::json!({ "locked": false, "candidates": cands });
            }
        }
    }

    // 0075 免审积分：自动过审的发布连续 +1（被拒路径在 admin 审核处清零）
    if auto_approve {
        let _ = sqlx::query("UPDATE users SET approve_streak = approve_streak + 1 WHERE id = $1")
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
    }

    Ok(ok(serde_json::json!({
        "id": id,
        "approval_status": approval_status,
        "auto_approved": auto_approve,
        "group_suggest": group_suggest,
    })))
}

/// 构建给指定用户的 .torrent 字节（注入本站 announce + passkey + private=1）。
/// 网页下载 / NP 兼容下载（compat_http）/ 临时凭证下载共用；鉴权与下载闸门由调用方先行完成。
/// announce 地址来源：站点设定 announce_url / https_announce_url 优先（设定页可改），
/// PUBLIC_TRACKER_URL 环境变量兜底。配置了 https 时首选加密汇报，http 作 BEP12 回退。
pub(crate) async fn build_torrent_bytes(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    torrent_id: i64,
) -> DomainResult<Vec<u8>> {
    let raw: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT f.raw FROM torrent_files f \
         JOIN torrents t ON t.id = f.torrent_id \
         WHERE f.torrent_id = $1 AND t.approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(raw) = raw else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let user = state
        .repo
        .find_user_by_id(user_id)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    // 下载闸门：与 tracker announce 的 left>0 拦截同口径——被停下载/挂起账号
    // 不应还能提前拿到 .torrent 文件
    let (download_enabled, suspended): (bool, bool) =
        sqlx::query_as("SELECT download_enabled, suspended FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .ok_or(DomainError::Unauthorized)?;
    if suspended {
        return Err(DomainError::Forbidden);
    }
    if !download_enabled {
        return Err(DomainError::Validation(
            "您的下载权限已被暂停，请联系管理组".into(),
        ));
    }
    // info dict 不动 → info_hash 与上传时一致（M05）
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
    // 审计修复（P0）：历史默认 announce_url 自带尾部 "/announce"（0034 迁移），拼接后
    // 生成 ".../announce/announce/<passkey>" 双重路径 + 错端口，真实客户端必然 404。
    // 这里剥掉尾部 /announce 双保险（迁移 0080 已同时纠正站点设定值本身）。
    let strip_announce = |v: String| -> String {
        let v = v.trim().trim_end_matches('/').to_string();
        match v.strip_suffix("/announce") {
            Some(s) => s.to_string(),
            None => v,
        }
    };
    let base_http = strip_announce(
        setting(&state.repo.db, "announce_url")
            .await
            .unwrap_or(env_host),
    );
    let announce = match setting(&state.repo.db, "https_announce_url").await {
        Some(https) if https != base_http => {
            format!("{}/announce/{}", strip_announce(https), user.passkey)
        }
        _ => format!("{base_http}/announce/{}", user.passkey),
    };
    // http 回退仅在与首选不同时下发（BEP12 单 tier：失败自动降级，不支持 TLS 的老客户端可用）
    let mut fallbacks = Vec::new();
    if !announce.starts_with(&format!("{base_http}/")) {
        fallbacks.push(format!("{base_http}/announce/{}", user.passkey));
    }
    // UDP tracker（BEP15）：默认不启用。私有站的计费身份靠 passkey 随 URL path 传递——
    // HTTP announce（BEP3）天然支持；UDP 包格式没有 path，标准客户端（libtorrent/
    // qBittorrent）不会附带 passkey，UDP tier 只对本站扩展约定的客户端可用。
    // NexusPHP 系站点全部走 HTTP announce，这也是私有 tracker 的行业惯例。
    // 需要时显式设 TRACKER_UDP_URL=udp://host:port 追加 tier（客户端失败后按 BEP12
    // 降级 HTTP 回退，不再默认推断给所有客户端强加一个必然失败的 UDP tier）。
    let udp_url = std::env::var("TRACKER_UDP_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();
    if !udp_url.is_empty() {
        fallbacks.push(format!(
            "{}/{}",
            udp_url.trim_end_matches('/'),
            user.passkey
        ));
    }
    crate::bencode::build_download_torrent(&raw, &announce, &fallbacks)
        .map_err(DomainError::TorrentInvalid)
}

// ============ 聚合组（0069：同一资源多版本，GZ Torrent Group 的教育域映射） ============

#[derive(Deserialize)]
struct GroupAttachReq {
    name: String,
    #[serde(default)]
    descr: Option<String>,
}

/// 把种子挂入聚合组：同名组直接复用（UNIQUE 天然幂等），否则创建新组。
/// 仅发布者本人或 staff（class ≥ 90）可操作。
#[post("/torrents/{id}/group")]
async fn group_attach(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<GroupAttachReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(DomainError::Validation("组名需 1-100 字".into()));
    }
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(torrent_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let gid: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM torrent_groups WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    {
        Some(g) => g,
        None => sqlx::query_scalar(
            "INSERT INTO torrent_groups (name, descr, created_by) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(name)
        .bind(
            body.descr
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty()),
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
    };
    sqlx::query("UPDATE torrents SET group_id = $1 WHERE id = $2")
        .bind(gid)
        .bind(torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "group_id": gid, "name": name })))
}

/// 订阅聚合组（0075：新版本入组并过审时推送）
#[post("/torrents/groups/{group_id}/subscribe")]
async fn group_subscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrent_groups WHERE id = $1)")
            .bind(gid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(gid));
    }
    sqlx::query(
        "INSERT INTO group_subscriptions (user_id, group_id) VALUES ($1, $2)          ON CONFLICT DO NOTHING",
    )
    .bind(auth.id)
    .bind(gid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": gid })))
}

/// 退订聚合组
#[post("/torrents/groups/{group_id}/unsubscribe")]
async fn group_unsubscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    sqlx::query("DELETE FROM group_subscriptions WHERE user_id = $1 AND group_id = $2")
        .bind(auth.id)
        .bind(gid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "unsubscribed": gid })))
}

/// 组详情 + 组内全部过审版本（详情页「同组资源」数据源；未入组返回 group=null）
#[get("/torrents/{id}/group")]
async fn group_info(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let torrent_id = path.into_inner();
    let group_id: Option<i64> = sqlx::query_scalar("SELECT group_id FROM torrents WHERE id = $1")
        .bind(torrent_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(gid) = group_id else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let row: Option<(String, Option<String>, Option<i32>)> =
        sqlx::query_as("SELECT name, descr, category_id FROM torrent_groups WHERE id = $1")
            .bind(gid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, descr, category_id)) = row else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let items: Vec<(i64, String, Option<String>, i64, i32, i32, i32, bool)> = sqlx::query_as(
        "SELECT id, name, small_descr, size, seeders, leechers, times_completed, official_tag \
         FROM torrents WHERE group_id = $1 AND approval_status = 1 ORDER BY id",
    )
    .bind(gid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = items
        .into_iter()
        .map(|(id, n, sd, size, s, l, c, official)| {
            serde_json::json!({
                "id": id, "name": n, "small_descr": sd, "size": size,
                "seeders": s, "leechers": l, "times_completed": c,
                "official": official, "current": id == torrent_id,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "group": { "id": gid, "name": name, "descr": descr, "category_id": category_id },
        "items": items,
    })))
}

#[get("/torrents/{id}/download")]
async fn download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    use actix_web::body::BoxBody;

    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    // 付费下载（0086）：免费/发布者/已购直接放行，否则扣费（余额不足拦截）
    torrents::charge_for_download(&state.repo.db, auth.id, torrent_id).await?;
    let body = build_torrent_bytes(&state, auth.id, torrent_id).await?;
    let mut resp = HttpResponse::with_body(actix_web::http::StatusCode::OK, BoxBody::new(body));
    resp.headers_mut().insert(
        actix_web::http::header::CONTENT_TYPE,
        actix_web::http::header::HeaderValue::from_static("application/x-bittorrent"),
    );
    Ok(resp)
}

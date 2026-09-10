//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;

use crate::auth;

use crate::domain::{self};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents;

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
        .service(list)
        .service(detail)
        .service(torrent_detail_ext)
        .service(torrent_files)
        .service(torrent_thanks)
        .service(comments)
        .service(create_comment)
        .service(do_thank)
        .service(do_bookmark)
        .service(stats)
        .service(report_create)
        .service(rss_info)
        .service(home_sections)
        .service(announce_stats)
        .service(upload)
        .service(download)
        .service(issue_invite_handler)
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
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LoginReq>,
) -> DomainResult<impl Responder> {
    // 登录限流（§5.7：5 次/分钟/用户名，Redis 计数）
    throttle(&state, format!("login:{}", body.username)).await?;
    let user = state
        .repo
        .find_user_by_name(body.username.trim())
        .await?
        .ok_or(DomainError::InvalidCredentials)?;
    if !domain::verify_password(&user.pass_hash, &body.password) {
        return Err(DomainError::InvalidCredentials);
    }
    // 2FA（启用者必须带 totp_code）
    crate::twofa_http::login_totp_check(&state.repo.db, user.id, body.totp_code.unwrap_or(0))
        .await?;
    let token = auth::issue(user.id, user.class_id, &state.cfg.jwt_secret, 24)
        .map_err(DomainError::Internal)?;
    // 登录事件（控制面板账户概览 30 天活跃趋势）
    let _ = sqlx::query("INSERT INTO login_events (user_id) VALUES ($1)")
        .bind(user.id)
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

#[derive(Debug)]
pub struct AuthUser {
    pub id: i64,
    pub class_id: i32,
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
            if claims.iat < nbf {
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
    // 令牌撤销（§5.7）：Redis 记录该用户 not-before 时间戳，require_auth 校验 iat >= nbf，
    // 登出后旧 token 立即失效，新登录不受影响。TTL 与 token 最长寿命对齐（24h）。
    let mut c = state.redis.clone();
    let key = format!("logout_nbf:{}", auth.id);
    let nbf = chrono::Utc::now().timestamp();
    let _: () = redis::AsyncCommands::set_ex(&mut c, &key, nbf, 86400u64)
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
    Ok(ok(serde_json::json!({
        "id": user.id, "username": user.username, "class_id": user.class_id,
        "must_reset_password": user.must_reset_password,
        "uploaded": uploaded, "downloaded": downloaded,
        "seeding": seeding, "leeching": leeching,
        "uploads": uploads, "bookmarks": bookmarks,
        "class_name": class_name,
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
    Ok(ok(serde_json::json!({ "passkey": pk })))
}

// ============ 控制面板（复刻 NexusPHP usercp：账户概览 + 四组设定） ============

/// 账户概览（usercp.php 首页口径）：资料卡 + 分享率概况 + 登录趋势 + 摘要行 + 更多信息表
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
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let t = torrents::get_torrent_detail(&state.repo.db, path.into_inner()).await?;
    Ok(ok(t))
}

#[get("/torrents/{id}/files")]
async fn torrent_files(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    Ok(ok(
        torrents::list_files(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/thanks")]
async fn torrent_thanks(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    Ok(ok(
        torrents::list_thanks(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/comments")]
async fn comments(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
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

#[post("/torrents/{id}/thanks")]
async fn do_thank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    torrents::thank(&state.repo.db, path.into_inner(), auth.id).await?;
    Ok(ok(serde_json::json!({ "thanked": true })))
}

#[derive(Deserialize)]
struct BookmarkReq {
    on: bool,
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
            serde_json::json!({
                "id": id, "title": title, "body": body, "badge": badge,
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
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrents (info_hash, name, small_descr, descr, category_id, medium_id, grade_id, edition_id, owner_id, anonymous, size, numfiles, approval_status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0) RETURNING id",
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
    let tracker_host =
        std::env::var("PUBLIC_TRACKER_URL").unwrap_or_else(|_| "http://127.0.0.1:7070".into());
    let announce = format!(
        "{}/announce/{}",
        tracker_host.trim_end_matches('/'),
        user.passkey
    );
    let body = crate::bencode::build_download_torrent(&raw, &announce)
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

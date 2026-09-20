//! 注册（M01）：POST /auth/register。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, ip_banned, throttle};
use crate::state::AppState;

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

#[derive(sqlx::FromRow, serde::Serialize)]
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

#[derive(Deserialize)]
struct UserTorrentlistQuery {
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct RecentComment {
    torrent_id: i64,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
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

#[derive(Deserialize)]
struct PasswordChangeReq {
    old_password: String,
    new_password: String,
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    password: String,
    #[serde(default)]
    totp_code: Option<u32>,
}

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
pub async fn register(
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
        || !crate::gaps_http::captcha_verify(
            &state,
            &body.captcha_id,
            body.captcha_answer,
        )
        .await
    {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    // 注册限流（§5.7）：按来源 IP 每分钟 5 次，防邀请码爆破
    let ip = client_ip(&req);
    // IP 封禁强制校验（与登录同口径；封禁名单由管理面维护）
    if ip_banned(&state, &ip).await {
        return Err(DomainError::Validation(
            "IP 已被封禁，请联系管理组".into(),
        ));
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
    // 单事务注册（repo 分层收编）：事务体在 repo/auth.rs register_user——
    // 建用户+消费邀请码+绑邀请人原子完成（审计修复口径不变），
    // 邀请码无效整体回滚并区分 InviteUsed/InviteInvalid。
    let user_repo = crate::repo::auth::AuthRepo {
        db: state.repo.db.clone(),
    };
    let (user_id, _inviter) = user_repo
        .register_user(
            &new_user,
            &pass_hash,
            body.invite_code.trim(),
            reg_mode == "invite_only",
        )
        .await?;
    state
        .repo
        .audit(Some(user_id), "user_register", Some(user_id))
        .await;
    Ok(ok(serde_json::json!({ "user_id": user_id })))
}

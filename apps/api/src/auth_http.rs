//! 认证与用户自助（M01）：注册/登录/登出、me 系列、公开主页、抓取列表、设置。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第四段）。
//! 鉴权设施本体（require_auth/client_ip/UserStatusCache/throttle/ip_banned）
//! 仍留在 http.rs（全仓共用）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::Row;

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::SnatchRow;
use crate::http::{
    bump_guard_ver, client_ip, ip_banned, require_auth, throttle,
};
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

#[post("/auth/login")]
pub async fn login(
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
        return Err(DomainError::Validation(
            "IP 已被封禁，请联系管理组".into(),
        ));
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
    if let Err(e) = crate::twofa_http::login_totp_check(
        &state.repo.db,
        user.id,
        body.totp_code.unwrap_or(0),
    )
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
            "账号因长期未登录已被停用，请通过『联系我们』附上用户名申请恢复"
                .into(),
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
    // 移除。cookie 必须根路径：Next RSC 服务端渲染页面时（/users/1 等）要读
    // flux_token 转发 Bearer 给 API，而页面请求路径不在 /api/v1 下——浏览器按
    // Path 属性不会携带该 cookie，服务端 cookies() 读不到 → 401 → notFound()。
    // HttpOnly 仍使 XSS 无法读取（require_auth 同时接受 Cookie，见 token_from_request）。
    let mut resp = ok(serde_json::json!({
        "token": token,
        "must_reset_password": user.must_reset_password,
        "user": { "id": user.id, "username": user.username, "class_id": user.class_id }
    }));
    let cookie = actix_web::cookie::Cookie::build("flux_token", token)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::hours(24))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .finish()
        .to_string();
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&cookie)
            .expect("cookie 串解析为 HeaderValue 必然成功"),
    );
    Ok(resp)
}

#[post("/auth/logout")]
pub async fn logout(
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
        .path("/")
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
#[derive(sqlx::FromRow, serde::Serialize)]
struct LoginEventRow {
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    ip: Option<String>,
    ok: bool,
    #[sqlx(default)]
    user_agent: String,
    /// 0=成功 1=密码错误 3=停用账号 4=未知用户名
    #[sqlx(default)]
    reason: i16,
}

/// 我的权限清单（前端 admin Tab 渲染过滤用；批量取回避免 N 次 round-trip）
#[get("/me/perms")]
pub async fn me_perms(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let keys =
        crate::authz::user_perm_keys(&state.repo.db, auth.class_id, auth.id)
            .await;
    let roles = crate::authz::user_role_keys(&state.repo.db, auth.id).await;
    Ok(ok(serde_json::json!({ "perms": keys, "roles": roles })))
}

#[get("/me")]
pub async fn me(
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
    let (
        uploaded,
        downloaded,
        seeding,
        leeching,
        uploads,
        bookmarks,
        class_name,
    ) = row.unwrap_or((0, 0, 0, 0, 0, 0, None));
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
    let (avatar_url, frame_id, frame_css, frame_image) =
        deco.unwrap_or((None, None, None, None));
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

#[get("/me/logins")]
pub async fn my_login_history(
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
pub async fn rotate_passkey(
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

#[post("/me/password/change")]
pub async fn me_password_change(
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
    let (pass_hash,): (String,) =
        sqlx::query_as("SELECT pass_hash FROM users WHERE id = $1")
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

#[get("/users/{id}")]
pub async fn user_public_profile(
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
    let (
        gender_code,
        _country,
        _isp,
        up_speed,
        down_speed,
        info,
        signature,
        online,
    ) = extra
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
    let (real_up, real_down, seed_seconds, _, seeding_size) =
        traffic.unwrap_or((0, 0, 0, 0, 0));
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
    let spark: (i64,) =
        sqlx::query_as("SELECT spark_balance FROM users WHERE id = $1")
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
    let invites_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invites WHERE inviter_id = $1 AND status = 0",
    )
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
    let achievements: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_achievements WHERE user_id = $1",
    )
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
        if let Some((cid, cname, need_up, need_dl, need_sh, need_age)) =
            rules.into_iter().next()
        {
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

#[get("/users/{id}/torrentlist")]
pub async fn user_torrentlist(
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
pub async fn me_overview(
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
    let get = |col: &str| -> serde_json::Value {
        r.try_get(col).unwrap_or(serde_json::Value::Null)
    };
    let get_opt_str = |col: &str| -> Option<String> {
        r.try_get::<Option<String>, _>(col).ok().flatten()
    };
    let get_i64 = |col: &str| -> i64 { r.try_get::<i64, _>(col).unwrap_or(0) };
    let get_bool =
        |col: &str| -> bool { r.try_get::<bool, _>(col).unwrap_or(false) };
    let get_str = |col: &str| -> String {
        r.try_get::<String, _>(col).unwrap_or_default()
    };
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
    let (next_name, next_req) =
        next.unwrap_or_else(|| ("Max".into(), uploaded.max(1)));

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

#[get("/me/settings")]
pub async fn me_settings_get(
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

/// 逐字段 COALESCE 更新（与 NexusPHP usercp save 一致：只改提交的字段）
#[put("/me/settings")]
pub async fn me_settings_put(
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

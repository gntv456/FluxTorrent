//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
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
        .service(rotate_passkey)
        .service(list)
        .service(detail)
        .service(comments)
        .service(create_comment)
        .service(do_thank)
        .service(do_bookmark)
        .service(stats)
        .service(announce_stats)
        .service(upload)
        .service(download)
        .service(issue_invite_handler)
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
    Ok(ok(serde_json::json!({
        "id": user.id, "username": user.username, "class_id": user.class_id,
        "must_reset_password": user.must_reset_password
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
    cursor: Option<String>,
    limit: Option<i64>,
}

#[get("/torrents")]
async fn list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    let filter = torrents::TorrentFilter {
        category_id: q.category_id,
        medium_id: q.medium_id,
        grade_id: q.grade_id,
        edition_id: q.edition_id,
        official: q.official,
        include_dead: q.include_dead.unwrap_or(false),
        search: q.search.as_deref().map(str::to_string),
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
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let t = torrents::get_torrent(&state.repo.db, path.into_inner()).await?;
    Ok(ok(t))
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
async fn stats(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    Ok(ok(torrents::site_stats(&state.repo.db).await?))
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
        "INSERT INTO torrents (info_hash, name, small_descr, category_id, medium_id, grade_id, edition_id, owner_id, anonymous, size, numfiles, approval_status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 0) RETURNING id",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&name)
    .bind(&form.small_descr)
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

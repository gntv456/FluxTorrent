//! me 基础面（M01）：perms/me/logins/passkey 轮换/改密码。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
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

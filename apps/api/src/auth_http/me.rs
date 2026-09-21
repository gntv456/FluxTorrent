//! me 基础面（M01）：perms/me/logins/passkey 轮换/改密码。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
        "SELECT count(*) FROM messages WHERE receiver_id = $1 AND \
         unread = true AND location = 1",
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

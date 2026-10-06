use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 后台用户详情（参考站 user/users/{id} 详情口径：字段全景 + 统计）
#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserDetail {
    id: i64,
    username: String,
    email: String,
    /// 掩码（0285）：明文 passkey 不再对任意 staff 下发，
    /// 取明文走 POST /admin/users/passkey/reveal（专项权限 + outranks + 审计）。
    passkey_masked: String,
    class_id: i32,
    class_name: Option<String>,
    title: Option<String>,
    uploaded: i64,
    downloaded: i64,
    spark_balance: i64,
    status: i16,
    download_enabled: bool,
    suspended: bool,
    parked: bool,
    donor: bool,
    totp_enabled: bool,
    invited_by: Option<i64>,
    inviter_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    seeding: i64,
    leeching: i64,
    uploads: i64,
    invites_unused: i64,
    // 详情页全景扩充（参考站 user-profile 口径）
    comments: i64,
    downloaded_count: i64,
    medals: i64,
    warned_until: Option<chrono::DateTime<chrono::Utc>>,
    warned_reason: Option<String>,
    last_ip: Option<String>,
    seed_seconds: i64,
    attendance_days: i64,
}

#[get("/admin/users/{id}")]
async fn user_admin_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let uid = path.into_inner();
    let row: Option<AdminUserDetail> = sqlx::query_as(
        r#"SELECT u.id, u.username, u.email,
                  left(u.passkey, 4) || '****' ||
                    right(u.passkey, 4) AS passkey_masked,
                  u.class_id, c.name AS class_name,
                  u.title, u.uploaded, u.downloaded, u.spark_balance, u.status,
                  u.download_enabled, u.suspended, u.parked, u.donor, u.totp_enabled,
                  u.invited_by, i.username AS inviter_name, u.created_at, u.last_seen_at,
                  (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
                  (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
                  (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id) AS uploads,
                  (SELECT count(*) FROM invites v WHERE v.inviter_id = u.id AND v.status = 0) AS invites_unused,
                  (SELECT count(*) FROM comments cm WHERE cm.user_id = u.id) AS comments,
                  (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS downloaded_count,
                  (SELECT count(*) FROM user_medals um WHERE um.user_id = u.id) AS medals,
                  u.warned_until, u.warned_reason,
                  host((SELECT l.ip FROM login_events l WHERE l.user_id = u.id ORDER BY l.id DESC LIMIT 1)) AS last_ip,
                  COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0)::bigint AS seed_seconds,
                  (SELECT count(*) FROM attendance a WHERE a.user_id = u.id) AS attendance_days
           FROM users u
           LEFT JOIN user_classes c ON c.id = u.class_id
           LEFT JOIN users i ON i.id = u.invited_by
           WHERE u.id = $1"#,
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let user = row.ok_or(DomainError::NotFound(uid))?;
    // 敏感读取留痕（0285）：会员详情含邮箱/_lastIP/流量/火花余额，
    // 旧版 staff 随便翻、事后无从追查「谁看过谁的档案」。
    state
        .repo
        .audit_detail(Some(auth.id), "user.detail.view", Some(uid), None, None)
        .await;
    Ok(ok(user))
}

/// 详情页「做种/下载」关联 tab：该用户的 snatches 全景（种子名/大小/状态/时长/H&R）
#[derive(sqlx::FromRow, serde::Serialize)]
struct UserSnatchRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeded_seconds: i32,
    seeding: bool,
    hr_flag: bool,
}

#[get("/admin/users/{id}/snatches")]
async fn user_admin_snatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<UserSnatchRow> = sqlx::query_as(
        "SELECT s.torrent_id, t.name, t.size, s.seeded_seconds, s.seeding, s.hr_flag \
         FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 \
         ORDER BY s.seeding DESC, s.seeded_seconds DESC LIMIT 200",
    )
    .bind(path.into_inner())
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

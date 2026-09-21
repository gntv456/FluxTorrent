use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 后台用户详情（参考站 user/users/{id} 详情口径：字段全景 + 统计）
#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserDetail {
    id: i64,
    username: String,
    email: String,
    passkey: String,
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
    let _auth = staff(&req, &state).await?;
    let uid = path.into_inner();
    let row: Option<AdminUserDetail> = sqlx::query_as(
        r#"SELECT u.id, u.username, u.email, u.passkey, u.class_id, c.name AS class_name,
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
                  COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0) AS seed_seconds,
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

/// 详情页授予勋章（参考站用户详情「授予勋章」口径）：管理发放 source='admin'
#[post("/admin/users/{id}/medal/{medal_id}")]
async fn user_grant_medal(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, medal_id) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM medals WHERE id = $1)")
            .bind(medal_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(medal_id));
    }
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'admin', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(uid)
    .bind(medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.grant_medal", Some(uid))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "medal_id": medal_id }),
    ))
}

/// 详情页授予道具/卡牌（参考站「授予道具」口径）：把商店道具（含化妆卡/改名卡等卡牌类）免费发放给目标用户。
/// 即时类（上传量/火花/邀请）直接生效；卡牌装饰类入 shop_orders（零元，source=admin）待用户使用。
#[post("/admin/users/{id}/grant-item/{item_id}")]
async fn user_grant_item(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, item_id) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let item: Option<(String, String, serde_json::Value)> =
        sqlx::query_as("SELECT name, kind, config FROM shop_items WHERE id = $1 AND active = true")
            .bind(item_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, config)) = item else {
        return Err(DomainError::NotFound(item_id));
    };
    match kind.as_str() {
        "upload_credit" => {
            let gb = config.get("gb").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(uid)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "gift_spark" => {
            let amount =
                config.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
            if amount > 0 {
                let idem = format!(
                    "admin_grant_item:{}:{}",
                    uid,
                    chrono::Utc::now().timestamp()
                );
                crate::economy_http::earn_spark(
                    &state.repo.db,
                    uid,
                    amount,
                    "admin_grant_item",
                    &idem,
                )
                .await?;
            }
        }
        "invite" | "temp_invite" => {
            sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + 1 WHERE id = $1",
            )
            .bind(uid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {
            let idem = format!(
                "admin_grant_item:{}:{}:{}",
                uid,
                item_id,
                chrono::Utc::now().timestamp()
            );
            sqlx::query(
                "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
                 VALUES ($1, $2, 0, $3, $4)",
            )
            .bind(uid)
            .bind(item_id)
            .bind(&idem)
            .bind(&config)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    state
        .repo
        .audit(Some(auth.id), "user.grant_item", Some(uid))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "item_id": item_id, "name": name, "kind": kind }),
    ))
}

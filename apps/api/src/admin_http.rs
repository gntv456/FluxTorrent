//! M29 完整管理后台 HTTP 接口（staff 专用）。
//!
//! 覆盖（方案 M29 验收口径的 Dev 版）：种子审核队列（通过/拒绝 + 理由）、
//! 举报处理、用户管理（封禁/解封/等级调整）、审计日志查询、站点运营概览。
//! 敏感操作全部 require_staff + audit 落库（§5.7）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_admin(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(admin_overview)
        .service(review_queue)
        .service(review_decide)
        .service(report_queue)
        .service(report_resolve)
        .service(user_admin_list)
        .service(user_admin_detail)
        .service(user_adjust)
        .service(user_flags)
        .service(user_set_status)
        .service(user_set_class)
        .service(audit_query)
        .service(staff_panel)
        .service(cheaters_scan)
        .service(site_settings_get)
        .service(site_settings_put)
        .service(agent_rules_list)
        .service(agent_rules_add)
        .service(agent_rules_del)
        .service(deny_reasons_list)
        .service(deny_reasons_add)
        .service(deny_reasons_update)
        .service(deny_reasons_delete)
        .service(admin_torrent_list)
        .service(torrent_op_logs)
        .service(admin_spark_logs)
        .service(admin_torrent_buys)
        .service(admin_login_logs)
        .service(forum_admin_list)
        .service(forum_admin_create)
        .service(forum_admin_update)
        .service(forum_admin_delete)
        .service(forum_mod_add)
        .service(forum_mod_remove)
}

async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    Ok(auth)
}

/// 运营概览：待审/举报/用户/种子计数
#[get("/admin/overview")]
async fn admin_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (pending, reports, users, torrents, banned): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM torrents WHERE approval_status = 0), \
            (SELECT count(*) FROM reports WHERE status = 0), \
            (SELECT count(*) FROM users), \
            (SELECT count(*) FROM torrents), \
            (SELECT count(*) FROM users WHERE status >= 2)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "pending_reviews": pending, "open_reports": reports,
        "users": users, "torrents": torrents, "banned_users": banned,
        "operator": auth.id,
    })))
}

// ============ 种子审核 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PendingTorrent {
    id: i64,
    name: String,
    owner_id: Option<i64>,
    size: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/reviews")]
async fn review_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<PendingTorrent> = sqlx::query_as(
        "SELECT id, name, owner_id, size, created_at FROM torrents \
         WHERE approval_status = 0 ORDER BY id LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ReviewReq {
    torrent_id: i64,
    approve: bool,
    #[serde(default)]
    reason: String,
    /// 拒绝原因字典（torrent_deny_reasons.id；好学站 torrent-deny-reasons 口径）
    #[serde(default)]
    deny_reason_id: Option<i64>,
}

#[post("/admin/reviews/decide")]
async fn review_decide(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let deny_reason_valid = if let Some(dr) = body.deny_reason_id {
        if body.approve {
            return Err(DomainError::Validation("通过时不需要拒绝原因".into()));
        }
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM torrent_deny_reasons WHERE id = $1 AND enabled",
        )
        .bind(dr)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("拒绝原因不存在或已停用".into()));
        }
        true
    } else {
        false
    };
    if !body.approve && !deny_reason_valid && body.reason.trim().is_empty() {
        return Err(DomainError::Validation("拒绝必须选择原因或填写理由".into()));
    }
    // 1=已过 2=被拒
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = $2, \
            deny_reason_id = $3, deny_note = $4 \
         WHERE id = $1 AND approval_status = 0",
    )
    .bind(body.torrent_id)
    .bind(if body.approve { 1 } else { 2 })
    .bind(if body.approve { None } else { body.deny_reason_id })
    .bind(if body.approve { None } else { Some(body.reason.trim().to_string()) })
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在待审状态".into()));
    }
    let action = if body.approve { "approve" } else { "reject" };
    // 种子操作记录（torrent-operation-logs 口径）
    let _ = sqlx::query(
        "INSERT INTO torrent_operation_logs (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, $3, $4::jsonb)",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(action)
    .bind(serde_json::json!({
        "reason": body.reason,
        "deny_reason_id": body.deny_reason_id,
    })
    .to_string())
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(
            Some(auth.id),
            if body.approve {
                "review.approve"
            } else {
                "review.reject"
            },
            Some(body.torrent_id),
        )
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id, "approved": body.approve, "reason": body.reason,
        "deny_reason_id": body.deny_reason_id,
    })))
}

// ============ 举报处理 ============

/// 举报队列：状态过滤 + 举报人 + 被举报对象上下文摘要
#[derive(sqlx::FromRow, serde::Serialize)]
struct ReportQueueRow {
    id: i64,
    reporter_id: i64,
    reporter_name: Option<String>,
    ref_type: String,
    ref_id: i64,
    ref_label: Option<String>,
    reason: String,
    status: i16,
    handled_name: Option<String>,
    handled_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/reports")]
async fn report_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ReportQueueQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let status_filter = q.status.as_deref().unwrap_or("pending");
    if !["pending", "handled", "all"].contains(&status_filter) {
        return Err(DomainError::Validation("status 需为 pending/handled/all".into()));
    }
    let rows: Vec<ReportQueueRow> = sqlx::query_as(
        "SELECT r.id, r.reporter_id, u.username AS reporter_name, r.ref_type, r.ref_id, r.reason, \
            r.status AS status, hu.username AS handled_name, r.handled_at, r.created_at, \
            CASE r.ref_type \
              WHEN 'torrent' THEN (SELECT t.name FROM torrents t WHERE t.id = r.ref_id) \
              WHEN 'user' THEN (SELECT ru.username FROM users ru WHERE ru.id = r.ref_id) \
              WHEN 'comment' THEN (SELECT left(c.body, 80) FROM comments c WHERE c.id = r.ref_id) \
              WHEN 'forum' THEN (SELECT left(p.body, 80) FROM posts p WHERE p.id = r.ref_id) \
              WHEN 'subtitle' THEN (SELECT s.title FROM subtitles s WHERE s.id = r.ref_id) \
            END AS ref_label \
         FROM reports r \
         LEFT JOIN users u ON u.id = r.reporter_id \
         LEFT JOIN users hu ON hu.id = r.handled_by \
         WHERE ($1 = 'all' AND TRUE) \
            OR ($1 = 'pending' AND r.status = 0) \
            OR ($1 = 'handled' AND r.status = 1) \
         ORDER BY r.id DESC LIMIT 200",
    )
    .bind(status_filter)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ReportQueueQuery {
    #[serde(default)]
    status: Option<String>,
}

/// 处置举报：action = act(已处置) | dismiss(驳回)；结果自动 PM 通知举报人
#[derive(Deserialize)]
struct ResolveReq {
    report_id: i64,
    #[serde(default)]
    action: Option<String>, // act / dismiss，缺省 act
    #[serde(default)]
    note: String,
}

#[post("/admin/reports/resolve")]
async fn report_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let action = body.action.as_deref().unwrap_or("act");
    if !["act", "dismiss"].contains(&action) {
        return Err(DomainError::Validation("action 需为 act/dismiss".into()));
    }
    // 取举报人与对象（供 PM）
    let info: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT reporter_id, ref_type, ref_id FROM reports WHERE id = $1 AND status = 0",
    )
    .bind(body.report_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((reporter_id, ref_type, ref_id)) = info else {
        return Err(DomainError::Validation("举报不存在或已处理".into()));
    };
    sqlx::query(
        "UPDATE reports SET status = 1, handled_by = $1, handled_at = now() \
         WHERE id = $2 AND status = 0",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // PM 通知举报人
    let subject = if action == "act" { "您的举报已处置" } else { "您的举报已审阅（未处置）" };
    let mut pm = format!(
        "您举报的对象（{} #{ref_id}）已被管理组审阅。\n\n结果：{}",
        ref_type,
        if action == "act" { "已按规则处置" } else { "经核实未违反规则，予以驳回" }
    );
    if !body.note.trim().is_empty() {
        pm.push_str("\n\n管理备注：");
        pm.push_str(body.note.trim());
    }
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(reporter_id)
    .bind(subject)
    .bind(pm)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "report.resolve", Some(body.report_id))
        .await;
    Ok(ok(serde_json::json!({ "resolved": body.report_id, "action": action })))
}

// ============ 用户管理（第五轮：好学站 /nexusphp user/users 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminUserListRow {
    id: i64,
    username: String,
    email: String,
    class_id: i32,
    class_name: Option<String>,
    uploaded: i64,
    downloaded: i64,
    status: i16,
    download_enabled: bool,
    suspended: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 五维筛选 + 排序 + 分页（ID/等级/状态/启用/下载权限/挂起 + 用户名/邮箱搜索）
#[derive(Deserialize)]
struct UserListQ {
    #[serde(default)]
    q: String,
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    class_id: Option<i32>,
    /// 0=全部 1=正常 2=禁言 3=封禁
    #[serde(default)]
    status: Option<i16>,
    /// yes/no/全部
    #[serde(default)]
    enabled: Option<String>,
    #[serde(default)]
    download: Option<String>,
    #[serde(default)]
    suspended: Option<String>,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_desc")]
    desc: bool,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}
fn default_sort() -> String {
    "id".into()
}
fn default_desc() -> bool {
    true
}
fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

#[get("/admin/users")]
async fn user_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserListQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    // 白名单排序字段（好学站可排序列：Id/等级/上传量/下载量/添加时间）
    let order_col = match q.sort.as_str() {
        "id" => "u.id",
        "class" => "u.class_id",
        "uploaded" => "u.uploaded",
        "downloaded" => "u.downloaded",
        "created" => "u.created_at",
        _ => "u.id",
    };
    // 条件占位符：$1=搜索 $2=ID $3=等级（WHERE 里按出现顺序绑定，见下方 bind 次序）
    let mut where_parts: Vec<String> = Vec::new();
    if q.id.is_some() {
        where_parts.push("u.id = $2".into());
    }
    if q.class_id.is_some() {
        where_parts.push("u.class_id = $3".into());
    }
    if let Some(st) = q.status {
        match st {
            0 => {}
            1 => where_parts.push("u.status = 0".into()),
            2 => where_parts.push("u.status = 1".into()),
            3 => where_parts.push("u.status >= 2".into()),
            _ => return Err(DomainError::Validation("status 取值 0-3".into())),
        }
    }
    if let Some(e) = q.enabled.as_deref() {
        match e {
            "yes" => where_parts.push("u.status < 2".into()),
            "no" => where_parts.push("u.status >= 2".into()),
            _ => {}
        }
    }
    if let Some(d) = q.download.as_deref() {
        match d {
            "yes" => where_parts.push("u.download_enabled".into()),
            "no" => where_parts.push("NOT u.download_enabled".into()),
            _ => {}
        }
    }
    if let Some(s) = q.suspended.as_deref() {
        match s {
            "yes" => where_parts.push("u.suspended".into()),
            "no" => where_parts.push("NOT u.suspended".into()),
            _ => {}
        }
    }
    if !q.q.trim().is_empty() {
        where_parts.push("(u.username ILIKE $1 OR u.email ILIKE $1)".into());
    }
    let where_sql = if where_parts.is_empty() {
        "TRUE".to_string()
    } else {
        where_parts.join(" AND ")
    };
    let dir = if q.desc { "DESC" } else { "ASC" };
    let sql = format!(
        r#"SELECT u.id, u.username, u.email, u.class_id, c.name AS class_name,
                  u.uploaded, u.downloaded, u.status, u.download_enabled, u.suspended,
                  u.created_at, u.last_seen_at
           FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
           WHERE {where_sql}
           ORDER BY {order_col} {dir}, u.id {dir}
           LIMIT $4 OFFSET $5"#,
    );
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<AdminUserListRow> = sqlx::query_as(&sql)
        .bind(pattern.clone())
        .bind(q.id)
        .bind(q.class_id)
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let count_sql = format!(
        "SELECT count(*) FROM users u WHERE {where_sql}"
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(pattern)
        .bind(q.id)
        .bind(q.class_id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows,
        "total": total,
        "page": q.page.max(1),
        "per_page": q.per_page,
    })))
}

/// 后台用户详情（好学站 user/users/{id} 详情口径：字段全景 + 统计）
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
                  (SELECT count(*) FROM invites v WHERE v.inviter_id = u.id AND v.status = 0) AS invites_unused
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

/// 「修改上传量等」（好学站用户详情按钮口径）：delta 语义，正加负减，下限 0
#[derive(Deserialize)]
struct UserAdjustReq {
    user_id: i64,
    #[serde(default)]
    uploaded_delta: Option<i64>,
    #[serde(default)]
    downloaded_delta: Option<i64>,
    #[serde(default)]
    spark_delta: Option<i64>,
    /// 增发邀请码数量（正数为增发，直接发放有效邀请）
    #[serde(default)]
    invite_grant: Option<i32>,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/users/adjust")]
async fn user_adjust(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserAdjustReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if body.uploaded_delta.is_none()
        && body.downloaded_delta.is_none()
        && body.spark_delta.is_none()
        && body.invite_grant.is_none()
    {
        return Err(DomainError::Validation("至少提供一项调整".into()));
    }
    // 数值调整仅 sysop/管理员（等级 93+）
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT uploaded, downloaded, spark_balance FROM users WHERE id = $1",
    )
    .bind(body.user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up0, down0, spark0) = row.ok_or(DomainError::NotFound(body.user_id))?;
    let up = ((up0 as i128 + body.uploaded_delta.unwrap_or(0) as i128).max(0)) as i64;
    let down = ((down0 as i128 + body.downloaded_delta.unwrap_or(0) as i128).max(0)) as i64;
    let spark = ((spark0 as i128 + body.spark_delta.unwrap_or(0) as i128).max(0)) as i64;
    sqlx::query(
        "UPDATE users SET uploaded = $2, downloaded = $3, spark_balance = $4 WHERE id = $1",
    )
    .bind(body.user_id)
    .bind(up)
    .bind(down)
    .bind(spark)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 火花调整写流水（余额权威在 spark_ledger；kind=admin，操作者入 ref_id）
    if let Some(delta) = body.spark_delta {
        if delta != 0 {
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'adjust', $3, $4, $5)",
            )
            .bind(body.user_id)
            .bind(delta)
            .bind(auth.id)
            .bind(format!(
                "admin-adjust-{}-{}",
                body.user_id,
                uuid::Uuid::new_v4().simple()
            ))
            .bind(spark)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    // 增发邀请：直接生成有效邀请码（30 天有效，NP takeinvite 口径）
    let mut granted_codes: Vec<String> = Vec::new();
    if let Some(grant) = body.invite_grant {
        if grant > 0 {
            for _ in 0..grant.min(50) {
                let code = uuid::Uuid::new_v4().simple().to_string();
                sqlx::query(
                    "INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, now() + interval '30 days')",
                )
                .bind(body.user_id)
                .bind(&code)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                granted_codes.push(code);
            }
        } else if grant < 0 {
            // 负数：回收最早到期的未用邀请
            sqlx::query(
                "DELETE FROM invites WHERE ctid IN (\
                    SELECT ctid FROM invites WHERE inviter_id = $1 AND status = 0 \
                    ORDER BY expires_at LIMIT $2)",
            )
            .bind(body.user_id)
            .bind((-grant) as i64)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    state
        .repo
        .audit(Some(auth.id), "user.adjust", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "uploaded": up, "downloaded": down, "spark": spark,
        "invite_grant": body.invite_grant.unwrap_or(0),
        "granted_codes": granted_codes,
        "note": body.note,
    })))
}

/// 下载权限 / 挂起 开关（tracker announce 执行点生效）
#[derive(Deserialize)]
struct UserFlagsReq {
    user_id: i64,
    #[serde(default)]
    download_enabled: Option<bool>,
    #[serde(default)]
    suspended: Option<bool>,
}

#[put("/admin/users/flags")]
async fn user_flags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserFlagsReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if body.download_enabled.is_none() && body.suspended.is_none() {
        return Err(DomainError::Validation("至少提供一个开关".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET \
            download_enabled = COALESCE($2, download_enabled), \
            suspended = COALESCE($3, suspended) \
         WHERE id = $1",
    )
    .bind(body.user_id)
    .bind(body.download_enabled)
    .bind(body.suspended)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "user.flags", Some(body.user_id))
        .await;
    // 挂起/禁下载变更需立即作用于 tracker（否则 passkey 缓存 60s 内仍有效）
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "download_enabled": body.download_enabled,
        "suspended": body.suspended,
    })))
}

#[derive(Deserialize)]
struct SearchQ {
    #[serde(default = "empty_q")]
    q: String,
}
fn empty_q() -> String {
    String::new()
}

#[derive(Deserialize)]
struct SetStatusReq {
    user_id: i64,
    /// 0 正常 1 禁言 2 封禁
    status: i16,
}

#[post("/admin/users/status")]
async fn user_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetStatusReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !(0..=2).contains(&body.status) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    let n = sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
        .bind(body.user_id)
        .bind(body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "user.set_status", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "status": body.status }),
    ))
}

#[derive(Deserialize)]
struct SetClassReq {
    user_id: i64,
    class_id: i32,
}

#[post("/admin/users/class")]
async fn user_set_class(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetClassReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 站长（99）才能调整等级；且禁止操作同级/更高级账户
    if auth.class_id < 99 || body.class_id >= 99 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("UPDATE users SET class_id = $2 WHERE id = $1 AND class_id < 99")
        .bind(body.user_id)
        .bind(body.class_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("用户不存在或不可调整".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "user.set_class", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "class_id": body.class_id }),
    ))
}

// ============ 审计日志 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AuditRow {
    id: i64,
    actor_id: Option<i64>,
    action: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/audit")]
async fn audit_query(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SearchQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, actor_id, action, created_at FROM audit_log \
         WHERE action ILIKE $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(pattern)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 管理组面板（staffpanel.php 复刻） + 站点设定 ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct StaffPanelEntry {
    panel: String,
    name: String,
    url: String,
    info: String,
}

/// 管理组面板三组入口：SysOp(99+) 全部；Administrator(93+) sysop+admin；版主(90+) 全部
#[get("/admin/staffpanel")]
async fn staff_panel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let visible_panels: &[&str] = if auth.class_id >= 99 {
        &["sysop", "admin", "moderator"]
    } else if auth.class_id >= 93 {
        &["admin", "moderator"]
    } else {
        &["moderator"]
    };
    let rows: Vec<StaffPanelEntry> = sqlx::query_as(
        "SELECT panel, name, url, info FROM staff_panel_entries \
         WHERE panel = ANY($1) ORDER BY panel, sort",
    )
    .bind(visible_panels.to_vec())
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "entries": rows,
        "role": if auth.class_id >= 99 { "sysop" }
            else if auth.class_id >= 93 { "administrator" }
            else { "moderator" },
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SiteSettingRow {
    name: String,
    value: String,
    updated_at: chrono::DateTime<chrono::Utc>,
    descr: Option<String>,
    grp: Option<String>,
}

/// 站点设定：sysop（99）只读 + 可写；administrator 只读
#[get("/admin/settings")]
async fn site_settings_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let rows: Vec<SiteSettingRow> = sqlx::query_as(
        "SELECT name, value, updated_at, descr, COALESCE(grp, 'misc') AS grp FROM site_settings ORDER BY grp, name",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "settings": rows,
        "editable": auth.class_id >= 99,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CheaterRow {
    user_id: i64,
    username: String,
    torrent_id: Option<i64>,
    name: Option<String>,
    upspeed: i64,
    uploaded_delta: i64,
    announced_at: chrono::DateTime<chrono::Utc>,
}

/// 作弊者探测（cheaters.php 口径）：snatches 实时上报速度 / 短窗上传增量超阈值的会话
#[get("/admin/cheaters")]
async fn cheaters_scan(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    // 阈值：上报速度 > 100 MB/s 或 1 小时内上传增量 > 500 GB 判为可疑
    // 口径：近 1 小时上传增量折算平均速度 > 100 MB/s，或 1 小时增量绝对值 > 500 GB
    let rows: Vec<CheaterRow> = sqlx::query_as(
        "SELECT s.user_id, u.username, s.torrent_id, t.name,             (g.uploaded_delta / 3600)::bigint AS upspeed,             COALESCE(g.uploaded_delta, 0)::bigint AS uploaded_delta,             now() AS announced_at          FROM snatches s          JOIN users u ON u.id = s.user_id          LEFT JOIN torrents t ON t.id = s.torrent_id          JOIN LATERAL (             SELECT COALESCE(sum(tl.delta_up), 0)::bigint AS uploaded_delta             FROM traffic_ledger tl             WHERE tl.user_id = s.user_id AND tl.torrent_id = s.torrent_id               AND tl.window_start > now() - interval '1 hour'          ) g ON TRUE          WHERE (s.seeding OR s.leeching)            AND (g.uploaded_delta > 536870912000                 OR g.uploaded_delta / 3600 > 104857600)          ORDER BY g.uploaded_delta DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct SettingPut {
    name: String,
    value: String,
}

/// 修改单项站点设定（仅 sysop）
#[put("/admin/settings")]
async fn site_settings_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettingPut>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 99 {
        return Err(DomainError::Forbidden);
    }
    if body.name.trim().is_empty() || body.value.len() > 4096 {
        return Err(DomainError::Validation("非法的设定项".into()));
    }
    let updated = sqlx::query(
        "UPDATE site_settings SET value = $3, updated_at = now() \
         WHERE name = $2 RETURNING name",
    )
    .bind(auth.id)
    .bind(body.name.trim())
    .bind(body.value.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.is_none() {
        return Err(DomainError::Validation("设定项不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "site_setting_update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ G-06 客户端黑白名单（NP AgentAllow/AgentDeny 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AgentRuleRow {
    id: i64,
    mode: String,
    pattern: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/agentrules")]
async fn agent_rules_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<AgentRuleRow> = sqlx::query_as(
        "SELECT r.id, r.mode, r.pattern, r.note, u.username AS created_by, r.created_at \
         FROM agent_rules r LEFT JOIN users u ON u.id = r.created_by \
         ORDER BY r.mode, r.id DESC LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AgentRuleReq {
    mode: String,
    pattern: String,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/agentrules")]
async fn agent_rules_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !["allow", "deny"].contains(&body.mode.as_str()) {
        return Err(DomainError::Validation("mode 取值 allow/deny".into()));
    }
    let p = body.pattern.trim();
    if p.is_empty() || p.len() > 100 {
        return Err(DomainError::Validation("pattern 长度 1-100".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO agent_rules (mode, pattern, note, created_by) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&body.mode)
    .bind(p)
    .bind(&body.note)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "agentrule.add", Some(id)).await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct AgentRuleDel {
    id: i64,
}

#[post("/admin/agentrules/delete")]
async fn agent_rules_del(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleDel>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query("DELETE FROM agent_rules WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state.repo.audit(Some(auth.id), "agentrule.del", Some(body.id)).await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}

// ============ 第五轮：拒绝原因字典（好学站 torrent-deny-reasons 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct DenyReasonRow {
    id: i64,
    sort: i32,
    reason: String,
    enabled: bool,
}

#[get("/admin/deny-reasons")]
async fn deny_reasons_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<DenyReasonRow> = sqlx::query_as(
        "SELECT id, sort, reason, enabled FROM torrent_deny_reasons ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct DenyReasonReq {
    reason: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
}

#[post("/admin/deny-reasons")]
async fn deny_reasons_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DenyReasonReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let r = body.reason.trim();
    if r.is_empty() || r.len() > 200 {
        return Err(DomainError::Validation("原因长度 1-200".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrent_deny_reasons (reason, sort, enabled) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(r)
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "deny_reason.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct DenyReasonPut {
    reason: Option<String>,
    sort: Option<i32>,
    enabled: Option<bool>,
}

#[put("/admin/deny-reasons/{id}")]
async fn deny_reasons_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<DenyReasonPut>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if let Some(r) = body.reason.as_deref() {
        if r.trim().is_empty() || r.len() > 200 {
            return Err(DomainError::Validation("原因长度 1-200".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE torrent_deny_reasons SET \
            reason = COALESCE($2, reason), sort = COALESCE($3, sort), enabled = COALESCE($4, enabled) \
         WHERE id = $1",
    )
    .bind(path.into_inner())
    .bind(body.reason.as_deref().map(str::trim))
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("拒绝原因不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "deny_reason.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/deny-reasons/{id}")]
async fn deny_reasons_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query("DELETE FROM torrent_deny_reasons WHERE id = $1")
        .bind(path.into_inner())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("拒绝原因不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "deny_reason.delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

// ============ 第五轮：后台种子管理列表（好学站 torrent/torrents 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminTorrentRow {
    id: i64,
    name: String,
    owner_id: Option<i64>,
    owner_name: Option<String>,
    category_id: i32,
    size: i64,
    seeders: i32,
    leechers: i32,
    approval_status: i16,
    deny_reason: Option<String>,
    deny_note: Option<String>,
    sticky: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct TorrentListQ {
    #[serde(default)]
    q: String,
    /// 0=全部 1=待审 2=通过 3=拒绝 4=死种
    #[serde(default)]
    status: Option<i16>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

#[get("/admin/torrents")]
async fn admin_torrent_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TorrentListQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let mut where_parts: Vec<String> = Vec::new();
    if !q.q.trim().is_empty() {
        where_parts.push("t.name ILIKE $1".into());
    }
    match q.status {
        Some(1) => where_parts.push("t.approval_status = 0".into()),
        Some(2) => where_parts.push("t.approval_status = 1".into()),
        Some(3) => where_parts.push("t.approval_status = 2".into()),
        Some(4) => where_parts.push("t.approval_status = 1 AND t.seeders = 0".into()),
        _ => {}
    }
    if let Some(c) = q.category_id {
        if c > 0 {
            where_parts.push("t.category_id = $2".into());
        }
    }
    let where_sql = if where_parts.is_empty() {
        "TRUE".to_string()
    } else {
        where_parts.join(" AND ")
    };
    let sql = format!(
        r#"SELECT t.id, t.name, t.owner_id, u.username AS owner_name, t.category_id,
                  t.size, t.seeders, t.leechers, t.approval_status,
                  dr.reason AS deny_reason, t.deny_note, t.sticky, t.created_at
           FROM torrents t
           LEFT JOIN users u ON u.id = t.owner_id
           LEFT JOIN torrent_deny_reasons dr ON dr.id = t.deny_reason_id
           WHERE {where_sql}
           ORDER BY t.id DESC LIMIT $3 OFFSET $4"#
    );
    let rows: Vec<AdminTorrentRow> = sqlx::query_as(&sql)
        .bind(crate::http::like_pattern(&q.q))
        .bind(q.category_id.filter(|c| *c > 0))
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows,
        "page": q.page.max(1),
        "per_page": q.per_page,
    })))
}

// ============ 第五轮：种子操作记录（好学站 torrent-operation-logs 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TorrentOpRow {
    id: i64,
    torrent_id: i64,
    torrent_name: Option<String>,
    operator_name: Option<String>,
    action: String,
    detail: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}


// ============ 第五轮：种子操作记录（好学站 torrent-operation-logs 口径） ============


#[derive(Deserialize)]
struct TorrentOpQ {
    #[serde(default)]
    torrent_id: Option<i64>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

#[get("/admin/torrent-ops")]
async fn torrent_op_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TorrentOpQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let (rows, total): (Vec<TorrentOpRow>, i64) = if let Some(tid) = q.torrent_id {
        let rows: Vec<TorrentOpRow> = sqlx::query_as(
            r#"SELECT l.id, l.torrent_id, t.name AS torrent_name, u.username AS operator_name,
                      l.action, l.detail, l.created_at
               FROM torrent_operation_logs l
               LEFT JOIN torrents t ON t.id = l.torrent_id
               LEFT JOIN users u ON u.id = l.operator_id
               WHERE l.torrent_id = $1
               ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(tid)
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrent_operation_logs WHERE torrent_id = $1",
        )
        .bind(tid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        (rows, total)
    } else {
        let rows: Vec<TorrentOpRow> = sqlx::query_as(
            r#"SELECT l.id, l.torrent_id, t.name AS torrent_name, u.username AS operator_name,
                      l.action, l.detail, l.created_at
               FROM torrent_operation_logs l
               LEFT JOIN torrents t ON t.id = l.torrent_id
               LEFT JOIN users u ON u.id = l.operator_id
               ORDER BY l.id DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM torrent_operation_logs")
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        (rows, total)
    };
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page,
    })))
}

// ============ 第五轮：记录查询（好学站 火花记录/种子购买/登录记录 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct SparkLogRow {
    id: i64,
    username: String,
    amount: i64,
    kind: String,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct RecordQ {
    #[serde(default)]
    q: String,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

#[get("/admin/spark-logs")]
async fn admin_spark_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<SparkLogRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, l.amount, l.kind, l.balance_after, l.created_at
           FROM spark_ledger l JOIN users u ON u.id = l.user_id
           WHERE u.username ILIKE $1
           ORDER BY l.created_at DESC, l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(pattern)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct TorrentBuyRow {
    id: i64,
    username: String,
    kind: String,
    ref_id: Option<i64>,
    amount: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/torrent-buys")]
async fn admin_torrent_buys(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<TorrentBuyRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, l.kind, l.ref_id, l.amount, l.created_at
           FROM spark_ledger l JOIN users u ON u.id = l.user_id
           WHERE l.kind IN ('torrent_buy', 'buy_torrent', 'token_buy') AND u.username ILIKE $1
           ORDER BY l.created_at DESC, l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(pattern)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LoginLogRow {
    id: i64,
    username: String,
    ip: Option<String>,
    ok: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/login-logs")]
async fn admin_login_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<LoginLogRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, host(l.ip) AS ip, l.ok, l.created_at
           FROM login_events l JOIN users u ON u.id = l.user_id
           WHERE u.username ILIKE $1 OR host(l.ip) ILIKE $1
           ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(pattern)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page })))
}


// ============ 论坛版块管理（forummanage.php 口径，forummanage ≥93） ============

/// 版块管理：三档门槛 + 受保护标记 + 版主名单
#[derive(serde::Serialize, sqlx::FromRow)]
struct ForumAdminRow {
    id: i64,
    name: String,
    descr: Option<String>,
    minclassread: i32,
    minclasswrite: i32,
    minclasscreate: i32,
    protected: bool,
    topics: i64,
}

#[get("/admin/forums")]
async fn forum_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    let rows: Vec<ForumAdminRow> = sqlx::query_as(
        "SELECT f.id, f.name, f.descr, f.minclassread, f.minclasswrite, f.minclasscreate, f.protected, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics \
         FROM forums f ORDER BY f.id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mods: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT fm.forum_id, u.id, u.username FROM forum_mods fm JOIN users u ON u.id = fm.user_id ORDER BY fm.forum_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "forums": rows, "mods": mods })))
}

#[derive(Deserialize)]
struct ForumUpsertReq {
    name: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    minclassread: Option<i32>,
    #[serde(default)]
    minclasswrite: Option<i32>,
    #[serde(default)]
    minclasscreate: Option<i32>,
    #[serde(default)]
    protected: Option<bool>,
}

fn forum_upsert_check(body: &ForumUpsertReq) -> DomainResult<()> {
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("版块名不能为空".into()));
    }
    let mr = body.minclassread.unwrap_or(0);
    let mw = body.minclasswrite.unwrap_or(0);
    let mc = body.minclasscreate.unwrap_or(0);
    if !(mr <= mw && mw <= mc) {
        return Err(DomainError::Validation("三档门槛需满足 读 ≤ 回 ≤ 发".into()));
    }
    Ok(())
}

#[post("/admin/forums")]
async fn forum_admin_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    forum_upsert_check(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO forums (name, descr, minclassread, minclasswrite, minclasscreate, min_class, protected) \
         VALUES ($1, $2, $3, $4, $5, $3, $6) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(&body.descr)
    .bind(body.minclassread.unwrap_or(0))
    .bind(body.minclasswrite.unwrap_or(0))
    .bind(body.minclasscreate.unwrap_or(0))
    .bind(body.protected.unwrap_or(false))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "forum.create", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/forums/{id}")]
async fn forum_admin_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    forum_upsert_check(&body)?;
    let fid = path.into_inner();
    let n = sqlx::query(
        "UPDATE forums SET name = $1, descr = $2, \
            minclassread = $3, minclasswrite = $4, minclasscreate = $5, \
            min_class = $3, protected = $6 \
         WHERE id = $7",
    )
    .bind(body.name.trim())
    .bind(&body.descr)
    .bind(body.minclassread.unwrap_or(0))
    .bind(body.minclasswrite.unwrap_or(0))
    .bind(body.minclasscreate.unwrap_or(0))
    .bind(body.protected.unwrap_or(false))
    .bind(fid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state.repo.audit(Some(auth.id), "forum.update", Some(fid)).await;
    Ok(ok(serde_json::json!({ "updated": fid })))
}

#[delete("/admin/forums/{id}")]
async fn forum_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    let fid = path.into_inner();
    let n = sqlx::query("DELETE FROM forums WHERE id = $1")
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state.repo.audit(Some(auth.id), "forum.delete", Some(fid)).await;
    Ok(ok(serde_json::json!({ "deleted": fid })))
}

/// 任命版主（无需等级）
#[derive(Deserialize)]
struct ForumModReq {
    username: String,
}

#[post("/admin/forums/{id}/mods")]
async fn forum_mod_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumModReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    let fid = path.into_inner();
    let uid: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
        .bind(&body.username)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::NotFound(0));
    };
    sqlx::query("INSERT INTO forum_mods (forum_id, user_id, created_by) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(fid)
        .bind(uid)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "forum.mod_add", Some(uid)).await;
    Ok(ok(serde_json::json!({ "forum_id": fid, "user_id": uid })))
}

#[delete("/admin/forums/{id}/mods/{user_id}")]
async fn forum_mod_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if auth.class_id < 93 {
        return Err(DomainError::Forbidden);
    }
    let (fid, uid) = path.into_inner();
    sqlx::query("DELETE FROM forum_mods WHERE forum_id = $1 AND user_id = $2")
        .bind(fid)
        .bind(uid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "forum.mod_remove", Some(uid)).await;
    Ok(ok(serde_json::json!({ "removed": uid })))
}

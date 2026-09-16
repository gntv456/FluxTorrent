//! M29 完整管理后台 HTTP 接口（staff 专用）。
//!
//! 覆盖（方案 M29 验收口径的 Dev 版）：种子审核队列（通过/拒绝 + 理由）、
//! 举报处理、用户管理（封禁/解封/等级调整）、审计日志查询、站点运营概览。
//! 敏感操作全部 require_staff + audit 落库（§5.7）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
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
        .service(report_claim)
        .service(report_release)
        .service(user_admin_list)
        .service(user_admin_detail)
        .service(user_admin_snatches)
        .service(user_grant_medal)
        .service(user_grant_item)
        .service(user_assign_jixiao)
        .service(user_admin_delete)
        .service(user_adjust)
        .service(user_flags)
        .service(user_set_status)
        .service(user_set_class)
        .service(role_list)
        .service(user_role_list)
        .service(user_role_grant)
        .service(user_role_revoke)
        .service(permission_matrix)
        .service(permission_matrix_update)
        .service(user_permission_view)
        .service(user_permission_set)
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL).await?;
    Ok(auth)
}

/// 权限护栏（NexusPHP checkPermission 口径）：操作者等级须**严格大于**目标用户。
///
/// 用于所有会修改目标用户的端点。此前 status / flags / adjust 仅有 staff(>=90) 守卫，
/// 导致总版主(93) 可封禁、挂起、扣减管理员(94) / 主管(95) / 站长(99) 的数据。
async fn ensure_outranks(
    db: &sqlx::PgPool,
    actor_class: i32,
    target_user_id: i64,
) -> DomainResult<()> {
    let target_class: Option<i32> = sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
        .bind(target_user_id)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let tc = target_class.ok_or(DomainError::NotFound(target_user_id))?;
    if actor_class <= tc {
        return Err(DomainError::Forbidden);
    }
    Ok(())
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
    /// 拒绝原因字典（torrent_deny_reasons.id；参考站 torrent-deny-reasons 口径）
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
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM torrent_deny_reasons WHERE id = $1 AND enabled")
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
    .bind(if body.approve {
        None
    } else {
        body.deny_reason_id
    })
    .bind(if body.approve {
        None
    } else {
        Some(body.reason.trim().to_string())
    })
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("种子不存在或不在待审状态".into()));
    }
    // 0075 免审积分：过审连击 +1 / 被拒清零（阈值放行在 upload 的 auto_approve 判定）
    // 0077 被拒禁发：累计 deny_count（阈值校验在 upload 前置）
    let _ = sqlx::query(
        "UPDATE users u SET approve_streak = CASE WHEN $2 THEN u.approve_streak + 1 ELSE 0 END,              deny_count = CASE WHEN $2 THEN u.deny_count ELSE u.deny_count + 1 END          FROM torrents t WHERE t.id = $1 AND u.id = t.owner_id",
    )
    .bind(body.torrent_id)
    .bind(body.approve)
    .execute(&state.repo.db)
    .await;
    // 0077 自动促销（U3D 口径）：过审时按 position 取第一条命中规则挂促销
    if body.approve {
        let _ = sqlx::query(
            r#"
            INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by)
            SELECT 'torrent', t.id, r.kind::promotion_kind_enum, now(),
                   now() + make_interval(hours => r.hours), 'task'::promotion_source, $2
            FROM torrents t
            JOIN auto_promo_rules r ON r.enabled
                 AND (r.name_regex = '' OR t.name ~* r.name_regex)
                 AND (r.min_size = 0 OR t.size >= r.min_size)
                 AND (r.max_size = 0 OR t.size < r.max_size)
                 AND (r.category_id IS NULL OR r.category_id = t.category_id)
            WHERE t.id = $1
            ORDER BY r.position LIMIT 1
            "#,
        )
        .bind(body.torrent_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
    }
    // 0075 组级订阅推送：入组种子过审 → 通知组订阅者（每人一信，含通知偏好过滤）
    if body.approve {
        let _ = sqlx::query(
            r#"
            INSERT INTO messages (sender_id, receiver_id, subject, body)
            SELECT NULL, gs.user_id, '订阅的聚合组有新版本',
                   format('你订阅的资源组「%s」有新种子过审：#%s %s。同类资源聚合页见种子详情。',
                          g.name, t.id, t.name)
            FROM torrents t
            JOIN torrent_groups g ON g.id = t.group_id
            JOIN group_subscriptions gs ON gs.group_id = g.id
            WHERE t.id = $1
              AND (u_notice_enabled(gs.user_id, 'group_new_version'))
            "#,
        )
        .bind(body.torrent_id)
        .execute(&state.repo.db)
        .await;
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
    .bind(
        serde_json::json!({
            "reason": body.reason,
            "deny_reason_id": body.deny_reason_id,
        })
        .to_string(),
    )
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
    if !["pending", "handling", "handled", "all"].contains(&status_filter) {
        return Err(DomainError::Validation(
            "status 需为 pending/handling/handled/all".into(),
        ));
    }
    let rows: Vec<ReportQueueRow> = sqlx::query_as(
        "SELECT r.id, r.reporter_id, u.username AS reporter_name, r.ref_type, r.ref_id, r.reason, \
            r.status AS status, hu.username AS handled_name, r.handled_at, r.created_at, \
            cu.username AS claimed_name, \
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
         LEFT JOIN users cu ON cu.id = r.claimed_by \
         WHERE ($1 = 'all' AND TRUE) \
            OR ($1 = 'pending' AND r.status = 0) \
            OR ($1 = 'handling' AND r.status = 2) \
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
        "SELECT reporter_id, ref_type, ref_id FROM reports WHERE id = $1 AND status IN (0, 2)",
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
         WHERE id = $2 AND status IN (0, 2)",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // PM 通知举报人
    let subject = if action == "act" {
        "您的举报已处置"
    } else {
        "您的举报已审阅（未处置）"
    };
    let mut pm = format!(
        "您举报的对象（{} #{ref_id}）已被管理组审阅。\n\n结果：{}",
        ref_type,
        if action == "act" {
            "已按规则处置"
        } else {
            "经核实未违反规则，予以驳回"
        }
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
    Ok(ok(
        serde_json::json!({ "resolved": body.report_id, "action": action }),
    ))
}

/// 认领举报（0 pending → 2 handling）：原子 CAS 防多人重复认领
#[derive(Deserialize)]
struct ReportClaimReq {
    report_id: i64,
}

#[post("/admin/reports/claim")]
async fn report_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE reports SET status = 2, claimed_by = $1, claimed_at = now() \
         WHERE id = $2 AND status = 0",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "举报不存在、已被认领或已处理".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "report.claim", Some(body.report_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.report_id })))
}

/// 释放认领（2 handling → 0 pending）：认领人本人可退回队列
#[post("/admin/reports/release")]
async fn report_release(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE reports SET status = 0, claimed_by = NULL, claimed_at = NULL \
         WHERE id = $1 AND status = 2 AND claimed_by = $2",
    )
    .bind(body.report_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("举报不在你名下或已结案".into()));
    }
    Ok(ok(serde_json::json!({ "released": body.report_id })))
}

// ============ 用户管理（第五轮：参考站 /nexusphp user/users 口径） ============

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
pub fn default_page() -> i64 {
    1
}
pub fn default_per_page() -> i64 {
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
    // 白名单排序字段（参考站可排序列：Id/等级/上传量/下载量/添加时间）
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
    let count_sql = format!("SELECT count(*) FROM users u WHERE {where_sql}");
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
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM medals WHERE id = $1)")
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
            sqlx::query("UPDATE users SET uploaded = uploaded + $2 WHERE id = $1")
                .bind(uid)
                .bind(gb * 1024 * 1024 * 1024)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "gift_spark" => {
            let amount = config.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
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
            sqlx::query("UPDATE users SET quota_extra = quota_extra + 1 WHERE id = $1")
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

/// 详情页分配考核（参考站「分配考核」口径）：把用户登记为某考核岗位（jixiao_claims，当期）
#[derive(Deserialize)]
struct AssignJixiaoReq {
    type_id: i64,
    /// YYYY-MM；缺省当月
    #[serde(default)]
    period: Option<String>,
    #[serde(default)]
    amount: Option<i64>,
}

#[post("/admin/users/{id}/jixiao")]
async fn user_assign_jixiao(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AssignJixiaoReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let uid = path.into_inner();
    // 等级护栏（审计修复）：90+ 可给 94/99 登记考核属越权，须严格高于目标
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jixiao_types WHERE id = $1)")
            .bind(body.type_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(body.type_id));
    }
    let period = body.period.clone().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND period = $3)",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("该用户本期已登记此考核岗位".into()));
    }
    sqlx::query(
        // metrics_snapshot 记 source='admin'：管理端登记与用户侧领取混在同一张表，
        // 用户侧 /jixiao/my 与达标月数加成（ops_http qualified_months）据此跳过 admin 行——
        // 「登记岗位」≠「领取工资」，admin 行不应推进 +10% 加成分子
        "INSERT INTO jixiao_claims (user_id, type_id, period, amount, metrics_snapshot) \
         VALUES ($1, $2, $3, $4, '{\"source\":\"admin\"}'::jsonb)",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .bind(body.amount.unwrap_or(0))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.assign_jixiao", Some(uid))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "type_id": body.type_id, "period": period }),
    ))
}

/// 单事务级联删除一个（已封禁）用户：覆盖全部 NO ACTION 引用列（83 处实测）。
/// 由 `DELETE /admin/users/{id}` 与批量 `deletedisabled`（http.rs）共用同一权威实现。
/// 任一步失败整体回滚；返回 anyhow::Result 便于批量调用方收集逐户失败原因。
pub async fn delete_user_cascade(db: &sqlx::PgPool, uid: i64) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // ── 软删化（账本纪律与风控追溯，0097）─────────────────────────────
    // 旧版物理 DELETE spark_ledger / login_events / hr_* / leak_events / snatches：
    // 违背「一切动账经 ledger 流水」的自家纪律（全局对账视图失真），且作弊者删号
    // 重注册后无历史证据可查。改为：
    //   1) users 行保留并匿名化（status=3 软删态；用户名/邮箱随机改写释放唯一索引；
    //      passkey 作废防 announce 冒用；pass_hash 置不可登录值）+ 撤销全部 JWT。
    //   2) 账本/风控证据表不再删除：spark_ledger、login_events、hr_snapshots、
    //      hr_violations、leak_events、snatches、traffic_ledger（cheat_events 本无 FK）。
    //   3) 社交/娱乐数据维持物理清理（无人引用、无需留痕）。
    let softened: usize = sqlx::query(
        r#"
        UPDATE users SET
            username = 'deleted-' || id::text || '-' || substr(md5(random()::text), 1, 8),
            email = 'deleted-' || id::text || '-' || substr(md5(random()::text), 1, 8) || '@deleted.invalid',
            pass_hash = '!', passkey = 'deleted-' || id::text,
            title = NULL, avatar_url = NULL, status = 3
        WHERE id = $1 AND status >= 2
        "#,
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?
    .rows_affected()
    .try_into()
    .unwrap_or(0);
    if softened == 0 {
        // 竞态：账号已被恢复/删除——按无行处理，幂等返回
        tx.commit().await?;
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, EXTRACT(EPOCH FROM now())::bigint)          ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf)",
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?;
    for sql in [
        // —— 本人拥有的业务数据（删；账本/风控证据除外，见函数头） ——
        "DELETE FROM attendance WHERE user_id = $1",
        "DELETE FROM task_claims WHERE user_id = $1",
        "DELETE FROM bank_demand_accounts WHERE user_id = $1",
        "DELETE FROM bank_deposits WHERE user_id = $1",
        "DELETE FROM bank_loans WHERE user_id = $1",
        "DELETE FROM bank_interest_records WHERE user_id = $1",
        "DELETE FROM messages WHERE sender_id = $1 OR receiver_id = $1",
        "DELETE FROM pmboxes WHERE user_id = $1",
        "DELETE FROM message_flood WHERE user_id = $1",
        "DELETE FROM staffmessages WHERE user_id = $1",
        "DELETE FROM comments WHERE user_id = $1",
        "DELETE FROM posts WHERE user_id = $1",
        "DELETE FROM topics WHERE user_id = $1",
        "DELETE FROM user_medals WHERE user_id = $1",
        "DELETE FROM user_roles WHERE user_id = $1",
        "DELETE FROM user_permissions WHERE user_id = $1",
        "DELETE FROM user_dressups WHERE user_id = $1",
        "DELETE FROM user_vouchers WHERE user_id = $1",
        "DELETE FROM bookmarks WHERE user_id = $1",
        "DELETE FROM api_tokens WHERE user_id = $1",
        "DELETE FROM password_resets WHERE user_id = $1",
        "DELETE FROM invites WHERE inviter_id = $1 OR used_by = $1",
        "DELETE FROM invite_quota WHERE user_id = $1",
        "DELETE FROM friendships WHERE user_id = $1 OR friend_id = $1",
        "DELETE FROM thanks WHERE user_id = $1",
        "DELETE FROM farm_plots WHERE user_id = $1",
        "DELETE FROM farm_harvests WHERE user_id = $1",
        "DELETE FROM fun_items WHERE user_id = $1",
        "DELETE FROM fun_item_votes WHERE user_id = $1",
        "DELETE FROM fun_votes WHERE user_id = $1",
        "DELETE FROM gomoku_games WHERE black_id = $1 OR white_id = $1",
        "DELETE FROM contest_entries WHERE user_id = $1",
        "DELETE FROM offers WHERE user_id = $1",
        "DELETE FROM offer_votes WHERE user_id = $1",
        "DELETE FROM requests WHERE user_id = $1",
        "DELETE FROM resub_uses WHERE user_id = $1",
        "DELETE FROM resurrections WHERE user_id = $1",
        "DELETE FROM subtitles WHERE user_id = $1",
        "DELETE FROM seed_milestones WHERE user_id = $1",
        "DELETE FROM jixiao_claims WHERE user_id = $1",
        "DELETE FROM appeals WHERE user_id = $1",
        "DELETE FROM download_keys WHERE user_id = $1",
        "DELETE FROM shop_orders WHERE user_id = $1",
        "DELETE FROM pool_donations WHERE user_id = $1",
        "DELETE FROM funding_contribs WHERE user_id = $1",
        "DELETE FROM push_subscriptions WHERE user_id = $1",
        "DELETE FROM reports WHERE reporter_id = $1",
        // —— 他方操作留痕列（置空，保留记录本身） ——
        "UPDATE torrents SET owner_id = NULL WHERE owner_id = $1",
        "UPDATE audit_log SET actor_id = NULL WHERE actor_id = $1",
        "UPDATE announcements SET author_id = NULL WHERE author_id = $1",
        "UPDATE appeals SET handled_by = NULL WHERE handled_by = $1",
        "UPDATE posts SET edited_by = NULL WHERE edited_by = $1",
        "UPDATE fundings SET creator_id = NULL WHERE creator_id = $1",
        "UPDATE hr_snapshots SET pardoned_by = NULL WHERE pardoned_by = $1",
        "UPDATE hr_violations SET resolved_by = NULL WHERE resolved_by = $1",
        "UPDATE leak_events SET resolved_by = NULL WHERE resolved_by = $1",
        "UPDATE reports SET claimed_by = NULL, handled_by = NULL WHERE claimed_by = $1 OR handled_by = $1",
        "UPDATE staffmessages SET answered_by = NULL, assigned_to = NULL WHERE answered_by = $1 OR assigned_to = $1",
        "UPDATE seed_preserve SET claimed_by = NULL WHERE claimed_by = $1",
        "UPDATE agent_rules SET created_by = NULL WHERE created_by = $1",
        "UPDATE email_bans SET created_by = NULL WHERE created_by = $1",
        "UPDATE forum_mods SET created_by = NULL WHERE created_by = $1",
        "UPDATE friend_links SET applied_by = NULL WHERE applied_by = $1",
        "UPDATE fun_polls SET created_by = NULL WHERE created_by = $1",
        "UPDATE ip_bans SET banned_by = NULL WHERE banned_by = $1",
        "UPDATE mass_mails SET sent_by = NULL WHERE sent_by = $1",
        "UPDATE promotions SET created_by = NULL WHERE created_by = $1",
        "UPDATE rules_revisions SET edited_by = NULL WHERE edited_by = $1",
        "UPDATE sticky_promotions SET created_by = NULL WHERE created_by = $1",
        "UPDATE torrent_groups SET created_by = NULL WHERE created_by = $1",
        "UPDATE torrent_operation_logs SET operator_id = NULL WHERE operator_id = $1",
        "UPDATE user_modify_logs SET modifier = NULL WHERE modifier = $1",
        "UPDATE username_change_logs SET operator = NULL WHERE operator = $1",
        "UPDATE users SET invited_by = NULL WHERE invited_by = $1",
    ] {
        sqlx::query(sql)
            .bind(uid)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?;
    }
    tx.commit().await?;
    Ok(())
}

/// 详情页删除用户（参考站用户详情「删除」口径）：仅 sysop，且要求先封禁（防误删活跃账号）
#[delete("/admin/users/{id}")]
async fn user_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_DELETE_DISABLED).await?;
    let uid = path.into_inner();
    if uid == auth.id {
        return Err(DomainError::Validation("不能删除自己".into()));
    }
    let status: Option<i16> = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    match status {
        None => return Err(DomainError::NotFound(uid)),
        Some(s) if s < 2 => {
            return Err(DomainError::Validation(
                "仅封禁状态的账号可删除，请先封禁".into(),
            ));
        }
        _ => {}
    }
    crate::admin_http::delete_user_cascade(&state.repo.db, uid)
        .await
        .map_err(|e| DomainError::Validation(e.to_string()))?;
    state
        .repo
        .audit(Some(auth.id), "user.delete", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": uid })))
}

/// 「修改上传量等」（参考站用户详情按钮口径）：delta 语义，正加负减，下限 0
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST).await?;
    // 数值调整同属伤害性操作，须严格高于目标等级
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let row: Option<(i64, i64, i64)> =
        sqlx::query_as("SELECT uploaded, downloaded, spark_balance FROM users WHERE id = $1")
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
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!(
            "数值调整 up{:+}/down{:+}/spark{:+}/invite{:+}",
            body.uploaded_delta.unwrap_or(0),
            body.downloaded_delta.unwrap_or(0),
            body.spark_delta.unwrap_or(0),
            body.invite_grant.unwrap_or(0)
        ),
    )
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_FLAGS).await?;
    if body.download_enabled.is_none() && body.suspended.is_none() {
        return Err(DomainError::Validation("至少提供一个开关".into()));
    }
    // 挂起 / 禁下载属伤害性操作，须严格高于目标等级
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
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
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!(
            "开关变更 download_enabled={:?} suspended={:?}",
            body.download_enabled, body.suspended
        ),
    )
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
    /// 操作理由（前端必填项；此前被 serde 静默丢弃）
    #[serde(default)]
    reason: String,
}

#[post("/admin/users/status")]
async fn user_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetStatusReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_STATUS).await?;
    if !(0..=2).contains(&body.status) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    // 操作者等级须严格大于目标用户（防同级/下级操作上级）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
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
    // 审计修复（P2）：封禁/禁言须即刻失效 tracker 的 passkey 缓存（≤60s 窗口内被 ban 用户
    // 仍可 announce）。flags 端点早有同款调用，此处此前遗漏。
    if body.status >= 1 {
        crate::http::bump_guard_ver(&state).await;
    }
    state
        .repo
        .audit(Some(auth.id), "user.set_status", Some(body.user_id))
        .await;
    let label = match body.status {
        1 => "禁言",
        2 => "封禁",
        _ => "恢复正常",
    };
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &if body.reason.trim().is_empty() {
            format!("状态 → {}", body.status)
        } else {
            format!("{label}：{}", body.reason.trim())
        },
    )
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CLASS).await?;
    // 不能把他人设为站长（与角色等级无关的结构性约束）
    if body.class_id >= 99 {
        return Err(DomainError::Forbidden);
    }
    // 与目标同高或更低不可调整（站长改自己也应被拦）
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
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
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("等级 → {}", body.class_id),
    )
    .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "class_id": body.class_id }),
    ))
}

// ============ 职务管理（user_roles，可兼任） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct RoleRow {
    key: String,
    name: String,
    descr: Option<String>,
}

/// 职务字典（管理界面渲染用）
#[get("/admin/roles")]
async fn role_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<RoleRow> =
        sqlx::query_as("SELECT key, name, descr FROM roles ORDER BY sort, key")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct UserRoleRow {
    user_id: i64,
    role_key: String,
    granted_by: Option<i64>,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct UserRoleQ {
    #[serde(default)]
    user_id: Option<i64>,
}

/// 用户持有的职务（指定 user_id 则只看该用户）
#[get("/admin/user-roles")]
async fn user_role_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserRoleQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<UserRoleRow> = match q.user_id {
        Some(uid) => {
            sqlx::query_as(
                "SELECT user_id, role_key, granted_by, granted_at, expires_at \
             FROM user_roles WHERE user_id = $1 ORDER BY granted_at",
            )
            .bind(uid)
            .fetch_all(&state.repo.db)
            .await
        }
        None => {
            sqlx::query_as(
                "SELECT user_id, role_key, granted_by, granted_at, expires_at \
             FROM user_roles ORDER BY granted_at DESC LIMIT 200",
            )
            .fetch_all(&state.repo.db)
            .await
        }
    }
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct GrantRoleReq {
    user_id: i64,
    role_key: String,
    /// RFC3339；缺省为永久
    #[serde(default)]
    expires_at: Option<String>,
}

/// 授予职务：须严格高于目标等级（与其它用户操作同一护栏）
#[post("/admin/user-roles")]
async fn user_role_grant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GrantRoleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let exists: Option<String> = sqlx::query_scalar("SELECT key FROM roles WHERE key = $1")
        .bind(&body.role_key)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("职务不存在".into()));
    }
    let expires = match &body.expires_at {
        Some(t) if !t.trim().is_empty() => Some(
            chrono::DateTime::parse_from_rfc3339(t)
                .map_err(|_| DomainError::Validation("到期时间格式无效".into()))?
                .with_timezone(&chrono::Utc),
        ),
        _ => None,
    };
    crate::authz::grant_role(
        &state.repo.db,
        body.user_id,
        &body.role_key,
        auth.id,
        expires,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.role_grant", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "role_key": body.role_key }),
    ))
}

/// 撤销职务
#[delete("/admin/user-roles/{user_id}/{role_key}")]
async fn user_role_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, String)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, role_key) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let n = crate::authz::revoke_role(&state.repo.db, uid, &role_key)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(uid));
    }
    state
        .repo
        .audit(Some(auth.id), "user.role_revoke", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

// ============ 权限配置（角色权限矩阵 + 用户级权限分配） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermRow {
    key: String,
    name: String,
    category: String,
    descr: Option<String>,
    /// 是否有真实生效点（false = 界面应标注「未接入」）
    implemented: bool,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermRoleRow {
    role_type: String,
    role_key: String,
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermGrantRow {
    role_type: String,
    role_key: String,
    permission_key: String,
}

/// 权限矩阵：权限清单 + 可配角色 + 当前授权映射
#[get("/admin/permission-matrix")]
async fn permission_matrix(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let permissions: Vec<PermRow> = sqlx::query_as(
        "SELECT key, name, category, descr, implemented FROM permissions \
             ORDER BY category, sort, key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 可配角色 = 已有配置的等级档 + 全部职务
    let roles: Vec<PermRoleRow> = sqlx::query_as(
        "SELECT t.role_type, t.role_key, COALESCE(uc.name, r.name, t.role_key) AS name
         FROM (
             SELECT DISTINCT 'class'::text AS role_type, role_key, 0 AS ord
             FROM role_permissions WHERE role_type = 'class'
             UNION ALL
             SELECT 'role', key, sort FROM roles
         ) t
         LEFT JOIN user_classes uc
                ON t.role_type = 'class'
               AND uc.id = CASE WHEN t.role_key ~ '^[0-9]+$'
                                THEN t.role_key::integer
                                ELSE -1 END
         LEFT JOIN roles r ON t.role_type = 'role' AND r.key = t.role_key
         ORDER BY t.role_type DESC, t.ord, t.role_key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let grants: Vec<PermGrantRow> = sqlx::query_as(
        "SELECT role_type, role_key, permission_key FROM role_permissions WHERE granted",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "permissions": permissions,
        "roles": roles,
        "grants": grants,
    })))
}

#[derive(Deserialize)]
struct PermMatrixItem {
    role_type: String,
    role_key: String,
    permission_key: String,
    granted: bool,
}

#[derive(Deserialize)]
struct PermMatrixReq {
    items: Vec<PermMatrixItem>,
}

/// 批量更新角色权限（勾选/取消）。仅站长可改，避免越权提权。
#[put("/admin/permission-matrix")]
async fn permission_matrix_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PermMatrixReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    if body.items.len() > 500 {
        return Err(DomainError::Validation("单次最多 500 项".into()));
    }
    let mut changed = 0u64;
    for it in &body.items {
        if !["class", "role"].contains(&it.role_type.as_str()) {
            return Err(DomainError::Validation("role_type 取值 class/role".into()));
        }
        let exists: Option<String> =
            sqlx::query_scalar("SELECT key FROM permissions WHERE key = $1")
                .bind(&it.permission_key)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation(format!(
                "权限不存在：{}",
                it.permission_key
            )));
        }
        let n = if it.granted {
            sqlx::query(
                "INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
                 VALUES ($1, $2, $3, true) ON CONFLICT DO NOTHING",
            )
            .bind(&it.role_type)
            .bind(&it.role_key)
            .bind(&it.permission_key)
            .execute(&state.repo.db)
            .await
        } else {
            sqlx::query(
                "DELETE FROM role_permissions
                 WHERE role_type = $1 AND role_key = $2 AND permission_key = $3",
            )
            .bind(&it.role_type)
            .bind(&it.role_key)
            .bind(&it.permission_key)
            .execute(&state.repo.db)
            .await
        }
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        changed += n;
    }
    state
        .repo
        .audit(Some(auth.id), "permission_matrix.update", None)
        .await;
    Ok(ok(serde_json::json!({ "changed": changed })))
}

#[derive(Deserialize)]
struct UserPermQ {
    user_id: i64,
}

/// 某用户的权限全貌：角色所得（effective）+ 用户级覆盖（overrides）
#[get("/admin/user-permissions")]
async fn user_permission_view(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserPermQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let class_id: Option<i32> = sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
        .bind(q.user_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(class_id) = class_id else {
        return Err(DomainError::NotFound(q.user_id));
    };
    let effective = crate::authz::user_perm_keys(&state.repo.db, class_id, q.user_id).await;
    let overrides = crate::authz::user_permission_overrides(&state.repo.db, q.user_id).await;
    let roles = crate::authz::user_role_keys(&state.repo.db, q.user_id).await;
    Ok(ok(serde_json::json!({
        "user_id": q.user_id,
        "class_id": class_id,
        "roles": roles,
        "effective": effective,
        "overrides": overrides.into_iter()
            .map(|(k, g)| serde_json::json!({ "permission_key": k, "granted": g }))
            .collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct SetUserPermReq {
    user_id: i64,
    permission_key: String,
    /// true=额外授予 / false=显式拒绝 / null=清除覆盖（继承角色）
    #[serde(default)]
    granted: Option<bool>,
    #[serde(default)]
    note: Option<String>,
}

/// 设置用户级权限覆盖（授予 / 拒绝 / 清除）
#[put("/admin/user-permissions")]
async fn user_permission_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetUserPermReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CLASS).await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let exists: Option<String> = sqlx::query_scalar("SELECT key FROM permissions WHERE key = $1")
        .bind(&body.permission_key)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("权限不存在".into()));
    }
    crate::authz::set_user_permission(
        &state.repo.db,
        body.user_id,
        &body.permission_key,
        body.granted,
        auth.id,
        body.note.as_deref(),
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.permission_set", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "permission_key": body.permission_key,
        "granted": body.granted,
    })))
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
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AUDIT_VIEW).await?;
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
    section: String,
    name: String,
    url: String,
    info: String,
    tab_key: String,
    min_class: i32,
}

/// 管理组面板：职能分组（section）+ 细粒度等级过滤（min_class）
/// 说明：旧的 panel 字段（sysop/admin/moderator）是权限等级，曾被当作分组维度，
/// 导致同一职能被拆散；现改为按 section 分组、按 min_class 逐条过滤，无权条目直接不返回。
#[get("/admin/staffpanel")]
async fn staff_panel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let rows: Vec<StaffPanelEntry> = sqlx::query_as(
        "SELECT section, name, url, info, tab_key, min_class FROM staff_panel_entries \
         WHERE min_class <= $1 ORDER BY section, sort, id",
    )
    .bind(auth.class_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "entries": rows,
        "role": if auth.class_id >= 99 { "sysop" }
            else if auth.class_id >= 93 { "administrator" }
            else { "moderator" },
        "class_id": auth.class_id,
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
    // 密文脱敏：旧版全量回值把 SMTP 密码等密文明文泄露给任何 90+。
    // 口径与新协议 settings/schema 一致——secret 字段只回「已设置」状态。
    let rows: Vec<SiteSettingRow> = sqlx::query_as(
        "SELECT s.name,                 CASE WHEN COALESCE(m.secret, false) THEN '' ELSE s.value END AS value,                 s.updated_at, s.descr, COALESCE(s.grp, 'misc') AS grp          FROM site_settings s LEFT JOIN settings_meta m ON m.name = s.name          ORDER BY s.grp, s.name",
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
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
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
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
    state
        .repo
        .audit(Some(auth.id), "agentrule.add", Some(id))
        .await;
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
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    let n = sqlx::query("DELETE FROM agent_rules WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state
        .repo
        .audit(Some(auth.id), "agentrule.del", Some(body.id))
        .await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}

// ============ 第五轮：拒绝原因字典（参考站 torrent-deny-reasons 口径） ============

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
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
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
    state
        .repo
        .audit(Some(auth.id), "deny_reason.add", Some(id))
        .await;
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
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
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
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    let rid = path.into_inner();
    // 审计修复（P1）：被种子引用（torrents.deny_reason_id del=a FK）时删除必 500。
    // 与 category_delete 同款前置护栏：有引用先解绑/换用别的理由。
    let refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE deny_reason_id = $1",
    )
    .bind(rid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if refs > 0 {
        return Err(DomainError::Validation(format!(
            "仍有 {refs} 个种子使用该拒绝理由（含已删除种子），请先改用其他理由"
        )));
    }
    let n = sqlx::query("DELETE FROM torrent_deny_reasons WHERE id = $1")
        .bind(rid)
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

// ============ 第五轮：后台种子管理列表（参考站 torrent/torrents 口径） ============

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
    /// 批量工作台：置顶状态与截止（第八轮）
    pos_state: i16,
    pos_state_until: Option<chrono::DateTime<chrono::Utc>>,
    /// 0 普通 1 推荐 2 经典
    pick_type: i16,
    /// 活动单种促销类型与截止（promotions scope='torrent'）
    promotion: Option<String>,
    promotion_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    /// H&R 标记（hr_policy.on）
    hr: bool,
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
    /// 发布者 UID
    #[serde(default)]
    owner: Option<i64>,
    /// 置顶筛选：1=置顶中 0=未置顶
    #[serde(default)]
    pos_state: Option<i16>,
    /// 单种促销：yes=促销中 no=无
    #[serde(default)]
    promo: Option<String>,
    /// 推荐 1 / 经典 2
    #[serde(default)]
    pick_type: Option<i16>,
    /// H&R 标记 yes/no
    #[serde(default)]
    hr: Option<String>,
    #[serde(default)]
    tag_id: Option<i32>,
    /// 体积区间（字节）
    #[serde(default)]
    size_min: Option<i64>,
    #[serde(default)]
    size_max: Option<i64>,
    /// 发布时间区间（RFC3339）
    #[serde(default)]
    created_from: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    created_to: Option<chrono::DateTime<chrono::Utc>>,
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
    // 全部谓词走「参数为 NULL 即放行」的固定形态，避免字符串拼接注入面
    let promo_on = match q.promo.as_deref() {
        Some("yes") => Some(true),
        Some("no") => Some(false),
        _ => None,
    };
    let hr_on = match q.hr.as_deref() {
        Some("yes") => Some(true),
        Some("no") => Some(false),
        _ => None,
    };
    let promo_exists = r#"(SELECT count(*) FROM promotions p WHERE p.starts_at <= now() AND p.ends_at > now() AND p.torrent_id = t.id) > 0"#;
    let where_sql = r#"(CASE WHEN $1 = 1 THEN t.approval_status = 0 WHEN $1 = 2 THEN t.approval_status = 1 WHEN $1 = 3 THEN t.approval_status = 2 WHEN $1 = 4 THEN t.approval_status = 1 AND t.seeders = 0 ELSE TRUE END)
           AND ($2::int IS NULL OR t.category_id = $2)
           AND ($3::bigint IS NULL OR t.owner_id = $3)
           AND ($4::smallint IS NULL OR t.pos_state = $4)
           AND ($5::bool IS NULL OR ($5 AND {promo_exists}) OR (NOT $5 AND NOT {promo_exists}))
           AND ($6::smallint IS NULL OR t.pick_type = $6)
           AND ($7::bool IS NULL OR COALESCE((t.hr_policy->>'on')::bool, FALSE) = $7)
           AND ($8::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $8))
           AND ($9::bigint IS NULL OR t.size >= $9)
           AND ($10::bigint IS NULL OR t.size <= $10)
           AND ($11::timestamptz IS NULL OR t.created_at >= $11)
           AND ($12::timestamptz IS NULL OR t.created_at <= $12)
           AND ($13::text IS NULL OR t.name ILIKE $13)"#;
    let where_sql = where_sql.replace("{promo_exists}", promo_exists);
    let sql = format!(
        r#"SELECT t.id, t.name, t.owner_id, u.username AS owner_name, t.category_id,
                  t.size, t.seeders, t.leechers, t.approval_status,
                  dr.reason AS deny_reason, t.deny_note, t.sticky,
                  t.pos_state, t.pos_state_until, t.pick_type,
                  (SELECT p.kind::text FROM promotions p
                     WHERE p.starts_at <= now() AND p.ends_at > now() AND p.torrent_id = t.id
                     ORDER BY p.id DESC LIMIT 1) AS promotion,
                  (SELECT p.ends_at FROM promotions p
                     WHERE p.starts_at <= now() AND p.ends_at > now() AND p.torrent_id = t.id
                     ORDER BY p.id DESC LIMIT 1) AS promotion_ends_at,
                  COALESCE((t.hr_policy->>'on')::bool, FALSE) AS hr,
                  t.created_at
           FROM torrents t
           LEFT JOIN users u ON u.id = t.owner_id
           LEFT JOIN torrent_deny_reasons dr ON dr.id = t.deny_reason_id
           WHERE {where_sql}
           ORDER BY t.id DESC LIMIT $14 OFFSET $15"#
    );
    let rows: Vec<AdminTorrentRow> = sqlx::query_as(&sql)
        .bind(q.status)
        .bind(q.category_id)
        .bind(q.owner)
        .bind(q.pos_state)
        .bind(promo_on)
        .bind(q.pick_type)
        .bind(hr_on)
        .bind(q.tag_id)
        .bind(q.size_min)
        .bind(q.size_max)
        .bind(q.created_from)
        .bind(q.created_to)
        .bind(if q.q.trim().is_empty() {
            None
        } else {
            Some(crate::http::like_pattern(&q.q))
        })
        .bind(q.per_page)
        .bind((q.page.max(1) - 1) * q.per_page)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let count_sql = format!("SELECT count(*) FROM torrents t WHERE {where_sql}");
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(q.status)
        .bind(q.category_id)
        .bind(q.owner)
        .bind(q.pos_state)
        .bind(promo_on)
        .bind(q.pick_type)
        .bind(hr_on)
        .bind(q.tag_id)
        .bind(q.size_min)
        .bind(q.size_max)
        .bind(q.created_from)
        .bind(q.created_to)
        .bind(if q.q.trim().is_empty() {
            None
        } else {
            Some(crate::http::like_pattern(&q.q))
        })
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "rows": rows,
        "page": q.page.max(1),
        "per_page": q.per_page,
        "total": total,
    })))
}

// ============ 第五轮：种子操作记录（参考站 torrent-operation-logs 口径） ============

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

// ============ 第五轮：种子操作记录（参考站 torrent-operation-logs 口径） ============

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
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM torrent_operation_logs WHERE torrent_id = $1")
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

// ============ 第五轮：记录查询（参考站 火花记录/种子购买/登录记录 口径） ============

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
    /// 用户 ID 精确过滤（详情页关联 tab 用）
    #[serde(default)]
    user_id: Option<i64>,
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
           WHERE u.username ILIKE $1 AND ($2::bigint IS NULL OR l.user_id = $2)
           ORDER BY l.created_at DESC, l.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(&pattern)
    .bind(q.user_id)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM spark_ledger l JOIN users u ON u.id = l.user_id \
         WHERE u.username ILIKE $1 AND ($2::bigint IS NULL OR l.user_id = $2)",
    )
    .bind(&pattern)
    .bind(q.user_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page, "total": total }),
    ))
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
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page }),
    ))
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
           WHERE (u.username ILIKE $1 OR host(l.ip) ILIKE $1)
             AND ($2::bigint IS NULL OR l.user_id = $2)
           ORDER BY l.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(&pattern)
    .bind(q.user_id)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // GeoIP（GeoLite2 离线库）：国家/城市随行返回；库缺失或内网 IP 时为 null
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let (iso, country, city) =
                r.ip.as_deref()
                    .map(crate::geo::lookup)
                    .unwrap_or((None, None, None));
            serde_json::json!({
                "id": r.id, "username": r.username, "ip": r.ip, "ok": r.ok,
                "country": iso, "country_name": country, "city": city,
                "created_at": r.created_at,
            })
        })
        .collect();
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM login_events l JOIN users u ON u.id = l.user_id \
         WHERE (u.username ILIKE $1 OR host(l.ip) ILIKE $1) AND ($2::bigint IS NULL OR l.user_id = $2)",
    )
    .bind(&pattern)
    .bind(q.user_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page, "total": total }),
    ))
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
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
        return Err(DomainError::Validation(
            "三档门槛需满足 读 ≤ 回 ≤ 发".into(),
        ));
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
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
    state
        .repo
        .audit(Some(auth.id), "forum.create", Some(id))
        .await;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
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
    state
        .repo
        .audit(Some(auth.id), "forum.update", Some(fid))
        .await;
    Ok(ok(serde_json::json!({ "updated": fid })))
}

#[delete("/admin/forums/{id}")]
async fn forum_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let force = q.get("force").map(|v| v == "true").unwrap_or(false);
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
    let fid = path.into_inner();
    // 审计修复（P1）：删除版块会静默级联删除其下全部主题与帖子（topics del=c → posts 级联）。
    // 非空版块要求显式 force=true 才执行，防误删整版内容。
    let topic_cnt: i64 = sqlx::query_scalar("SELECT count(*) FROM topics WHERE forum_id = $1")
        .bind(fid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if topic_cnt > 0 && !force {
        return Err(DomainError::Validation(format!(
            "该版块仍有 {topic_cnt} 个主题（删除将级联清空全部帖子）。确认知悉请传 force=true"
        )));
    }
    let n = sqlx::query("DELETE FROM forums WHERE id = $1")
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.delete", Some(fid))
        .await;
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
    let fid = path.into_inner();
    let uid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
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
    state
        .repo
        .audit(Some(auth.id), "forum.mod_add", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "forum_id": fid, "user_id": uid })))
}

#[delete("/admin/forums/{id}/mods/{user_id}")]
async fn forum_mod_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FORUMS_MANAGE).await?;
    let (fid, uid) = path.into_inner();
    sqlx::query("DELETE FROM forum_mods WHERE forum_id = $1 AND user_id = $2")
        .bind(fid)
        .bind(uid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.mod_remove", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "removed": uid })))
}

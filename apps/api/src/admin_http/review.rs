use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 种子审核 ============

/// 审核队列条目（0285 补内容面）。
/// 旧版只下发 id/name/owner_id/size/created_at —— 版主看不到简介/描述/文件数/截图/NFO/
/// 查重命中/上传者历史，却要为一条种子的生死按「过/不过」，实际等于只看标题审。
#[derive(sqlx::FromRow, serde::Serialize)]
struct PendingTorrent {
    id: i64,
    name: String,
    owner_id: Option<i64>,
    size: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    owner_name: Option<String>,
    category_name: Option<String>,
    small_descr: String,
    descr_excerpt: String,
    numfiles: i32,
    screenshots: i32,
    has_nfo: bool,
    has_media_info: bool,
    anonymous: bool,
    official_tag: bool,
    price: i64,
    dup_hash: i64,
    dup_name: i64,
    owner_approved: i64,
    owner_denied: i64,
}

/// 队列查询参数（0286 分页 / 筛选 / 排序）。
#[derive(Deserialize)]
struct QueueQuery {
    /// 排序：true（默认）= 最老优先，先到的先审；false = 最新优先
    #[serde(default)]
    oldest_first: Option<bool>,
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    owner_id: Option<i64>,
}

#[get("/admin/reviews")]
async fn review_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<QueueQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    // 旧口径是 `ORDER BY id LIMIT 200` 且无分页：实测积压 207 条时只回 200 条，
    // **最新提交的那几条根本不在返回里**（升序 + 截断 = 新种对审核员永久不可见）。
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);
    let oldest_first = q.oldest_first.unwrap_or(true);
    let filters = "WHERE t.approval_status = 0          AND ($1::int IS NULL OR t.category_id = $1)          AND ($2::bigint IS NULL OR t.owner_id = $2)".to_string();
    let order = if oldest_first {
        "ORDER BY t.created_at ASC, t.id ASC"
    } else {
        "ORDER BY t.created_at DESC, t.id DESC"
    };
    let sql = String::from(
        r#"SELECT t.id, t.name, t.owner_id, t.size, t.created_at,
                  u.username AS owner_name, c.name AS category_name,
                  COALESCE(t.small_descr, '') AS small_descr,
                  left(COALESCE(t.descr, ''), 800) AS descr_excerpt,
                  COALESCE(t.numfiles, 0)::int AS numfiles,
                  (CASE WHEN jsonb_typeof(t.screenshots) = 'array'
                        THEN jsonb_array_length(t.screenshots)
                        ELSE 0 END)::int AS screenshots,
                  (COALESCE(t.nfo, '') <> '') AS has_nfo,
                  (t.media_info IS NOT NULL) AS has_media_info,
                  COALESCE(t.anonymous, false) AS anonymous,
                  COALESCE(t.official_tag, false) AS official_tag,
                  COALESCE(t.price, 0)::bigint AS price,
                  (SELECT count(*) FROM torrents d
                    WHERE d.info_hash = t.info_hash
                      AND d.id <> t.id) AS dup_hash,
                  (SELECT count(*) FROM torrents d
                    WHERE d.id <> t.id
                      AND lower(d.name) = lower(t.name)) AS dup_name,
                  (SELECT count(*) FROM torrents w
                    WHERE w.owner_id = t.owner_id
                      AND w.approval_status = 1) AS owner_approved,
                  (SELECT count(*) FROM torrents w
                    WHERE w.owner_id = t.owner_id
                      AND w.approval_status = 2) AS owner_denied
           FROM torrents t
           LEFT JOIN categories c ON c.id = t.category_id
           LEFT JOIN users u ON u.id = t.owner_id
           {FILTERS}
           {ORDER}
           LIMIT $3 OFFSET $4"#,
    )
    .replace("{FILTERS}", &filters)
    .replace("{ORDER}", order);
    let rows: Vec<PendingTorrent> = sqlx::query_as(&sql)
        .bind(q.category_id)
        .bind(q.owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents t WHERE t.approval_status = 0          AND ($1::int IS NULL OR t.category_id = $1)          AND ($2::bigint IS NULL OR t.owner_id = $2)",
    )
    .bind(q.category_id)
    .bind(q.owner_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "total": total, "limit": limit, "offset": offset,
        "oldest_first": oldest_first, "items": rows,
    })))
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
    // 审核权独立判定（0285）：过去只判 staff.panel，审/改/删全挤在 torrent.manage。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    // 利益冲突拦截（0285 实测：class 91 发布员可审并放行自己的种）
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(body.torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if owner_id == Some(auth.id) {
        return Err(DomainError::Validation("不能审核自己发布的种子".into()));
    }
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
            return Err(DomainError::Validation(
                "拒绝原因不存在或已停用".into(),
            ));
        }
        true
    } else {
        false
    };
    if !body.approve && !deny_reason_valid && body.reason.trim().is_empty() {
        return Err(DomainError::Validation(
            "拒绝必须选择原因或填写理由".into(),
        ));
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
                "UPDATE users u SET approve_streak = CASE WHEN $2 THEN \
         u.approve_streak + 1 ELSE 0 END, \
         deny_count = CASE WHEN $2 THEN u.deny_count ELSE u.deny_count + 1 END FROM torrents t WHERE t.id = $1 AND u.id = t.owner_id",
    )
    .bind(body.torrent_id)
    .bind(body.approve)
    .execute(&state.repo.db)
    .await;
    // 过审副作用（自动促销 + 组订阅推送）：与批量裁决共用同一实现
    if body.approve {
        super::review_side_effects::apply_approval_side_effects(
            &state.repo.db,
            body.torrent_id,
            auth.id,
        )
        .await;
    }
    let action = if body.approve { "approve" } else { "reject" };
    // U2 §12.5 审核结果通知：取数与发送在 review_notify.rs（0285 拆出并修列名）
    super::review_notify::notify_review_result(
        &state,
        body.torrent_id,
        body.approve,
        &body.reason,
    )
    .await;
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

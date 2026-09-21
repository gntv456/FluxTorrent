use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
use super::user_list::{default_page, default_per_page};

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
    let promo_exists = r#"(SELECT count(*) FROM promotions p WHERE \
     p.starts_at <= now() AND p.ends_at > now() AND p.torrent_id = t.id) > 0"#;
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
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let count_sql =
        format!("SELECT count(*) FROM torrents t WHERE {where_sql}");
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
    let (rows, total): (Vec<TorrentOpRow>, i64) = if let Some(tid) =
        q.torrent_id
    {
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
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
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
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM torrent_operation_logs")
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        (rows, total)
    };
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page,
    })))
}

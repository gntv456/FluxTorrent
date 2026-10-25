//! 邀请码列表（0204 分页；2026-10-20 状态筛选）。
//! 从 invite_http.rs 外移：该文件已在 360 行基线，加筛选会顶破行数门禁。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Serialize, sqlx::FromRow)]
struct InviteRow {
    id: i64,
    code: String,
    status: i16,
    used_by: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
    /// NP invite.php 口径：发送对象邮箱（邮件邀请才有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    email: Option<String>,
    /// 邮件是否已投递（发送失败为 false，用户可重发）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    emailed: Option<bool>,
}

#[derive(Deserialize)]
struct ListInvitesQuery {
    page: Option<i64>,
    /// 0 待使用 / 1 已使用 / 2 已过期（缺省 = 全部）
    status: Option<i16>,
}

/// 筛选谓词与 /invites/status 的三个计数同一口径：已过期同时认
/// 存储态 status=2 与「未用且已过期」，否则角标数与筛选结果数对不上。
const STATUS_COND: &str = "($2::int2 IS NULL \
     OR ($2 = 0 AND i.status = 0 AND i.expires_at > now()) \
     OR ($2 = 1 AND i.status = 1) \
     OR ($2 = 2 AND (i.status = 2 \
         OR (i.status = 0 AND i.expires_at <= now()))))";

#[get("/invites")]
pub async fn list_invites_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ListInvitesQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if let Some(s) = q.status {
        if !(0..=2).contains(&s) {
            return Err(DomainError::Validation(
                "邀请状态筛选无效（0 待使用 / 1 已使用 / 2 已过期）".into(),
            ));
        }
    }
    // 0204 分页：此前 LIMIT 50 截断重度用户历史；页大小 50。
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per = 50i64;
    // status=2（已过期）在下发时即时折算：过期未用的码视为失效
    //（注册侧已拒绝 expires_at > now()）。
    let rows_sql = format!(
        "SELECT i.id, i.code, \
                (CASE WHEN i.status = 0 AND i.expires_at <= now() \
                      THEN 2 ELSE i.status END)::int2 AS status, \
                u.username AS used_by, i.expires_at, i.email, i.emailed \
         FROM invites i LEFT JOIN users u ON u.id = i.used_by \
         WHERE i.inviter_id = $1 AND {STATUS_COND} \
         ORDER BY i.id DESC LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query_as::<_, InviteRow>(&rows_sql)
        .bind(auth.id)
        .bind(q.status)
        .bind(per)
        .bind(crate::dto::page_offset(page, per))
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let total_sql = format!(
        "SELECT count(*) FROM invites i \
         WHERE i.inviter_id = $1 AND {STATUS_COND}"
    );
    let total: i64 = sqlx::query_scalar(&total_sql)
        .bind(auth.id)
        .bind(q.status)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "page": page,
        "per": per,
    })))
}

//! 被邀请人列表（0204 P0，NP invitee 口径）：上传/下载/状态。
//! 从 invite_http.rs 机械外移（该文件已超 300 行门禁）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::ListQuery;

/// 邮箱脱敏口径同个人主页（不回传 email）。
///
/// 2026-10-16 补分页：此前是硬编码 `LIMIT 200` 且**没有分页参数**——同一
/// 模块的邀请记录 0204 已经补过分页（50/页），被邀请人这条漏了，导致
/// 邀请超过 200 人的账号看不到更早的记录（静默截断）；前端还把那 200 行
/// 全平铺，实测页高 10269px。口径与 list_invites_handler 一致：per 固定 50。
#[get("/invites/invitees")]
pub async fn list_invitees_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ListQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per = 50i64;
    type Row = (
        i64,
        String,
        i16,
        Option<String>,
        i64,
        i64,
        chrono::DateTime<chrono::Utc>,
    );
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, Row>(
        "SELECT u.id, u.username, u.status, c.name AS class_name, \
                u.uploaded, u.downloaded, u.created_at \
         FROM users u LEFT JOIN user_classes c ON c.id = u.class_id \
         WHERE u.invited_by = $1 ORDER BY u.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.id)
    .bind(per)
    .bind(crate::dto::page_offset(page, per))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, username, status, class_name, up, down, created_at)| {
        serde_json::json!({
            "id": id, "username": username, "status": status,
            "class_name": class_name,
            "uploaded": up, "downloaded": down,
            "created_at": created_at.to_rfc3339(),
        })
    })
    .collect();
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE invited_by = $1")
            .bind(auth.id)
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

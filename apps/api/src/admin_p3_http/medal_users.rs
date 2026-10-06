//! P2-7 用户勋章持有管理
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{modify_log, staff};

#[derive(sqlx::FromRow, serde::Serialize)]
struct UserMedalRow {
    user_id: i64,
    username: String,
    medal_id: i64,
    medal_name: String,
    source: String,
    wearing: bool,
    granted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 到期时间（0291）：后台早就有「改期」按钮，列表却不下发这一列——
    /// 站长点改期之前看不到当前到期，等于闭眼改。NULL=永久。
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 已过期（0291）：有效期到 now 之前就不再生效，但行还在、
    /// 用户页与列表都读不出「这枚已经废了」。改期按钮要能一眼找到该点的行。
    expired: bool,
}

#[derive(Deserialize)]
struct UserMedalQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/user-medals")]
async fn admin_user_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserMedalQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<UserMedalRow> = sqlx::query_as(
        r#"SELECT um.user_id, u.username, um.medal_id, m.name AS medal_name,
                  um.source, um.wearing, um.granted_at, um.expires_at,
                  (um.expires_at IS NOT NULL AND um.expires_at < now())
                      AS expired
           FROM user_medals um
           JOIN users u ON u.id = um.user_id
           JOIN medals m ON m.id = um.medal_id
           WHERE ($1::bigint IS NULL OR um.user_id = $1)
           ORDER BY um.medal_id, um.user_id LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_medals um WHERE ($1::bigint IS NULL \
         OR um.user_id = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct UserMedalDel {
    user_id: i64,
    medal_id: i64,
}

/// 回收勋章（参考站 UserMedal 删除口径）
#[post("/admin/user-medals/delete")]
async fn admin_user_medal_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserMedalDel>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    // 等级护栏（0291）：回收属伤害性动作，与发放侧同闸——此前 93 可以直接摘掉
    // 站长(99) 的勋章，而给用户详情挂勋章都要严格高于目标。
    crate::admin_http::guard::ensure_outranks(
        &state.repo.db,
        auth.class_id,
        body.user_id,
    )
    .await?;
    let n = sqlx::query(
        "DELETE FROM user_medals WHERE user_id = $1 AND medal_id = $2",
    )
    .bind(body.user_id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.medal_id));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "medal.revoke",
            Some(body.user_id),
            None,
            Some(serde_json::json!({ "medal_id": body.medal_id })),
        )
        .await;
    modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("回收勋章 #{}", body.medal_id),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

//! 单发勋章（0286 从 user_detail.rs 拆出守 300 行门禁）：
//! 有效期覆盖（days）+ 附送 PM。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 详情页授予勋章（参考站用户详情「授予勋章」口径）：管理发放 source='admin'
#[post("/admin/users/{id}/medal/{medal_id}")]
pub(super) async fn user_grant_medal(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
    body: web::Json<GrantMedalReq>,
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
    // 有效期覆盖（0286 P3b）：缺省随勋章定义 duration_days（NULL=永久）；
    // 显式传 days（1-3650）则本次发放按覆盖值——活动限时勋章不再需要
    // 改勋章定义（改定义影响全体持有者的语义边界）
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'admin', \
                CASE WHEN $3::int IS NOT NULL \
                  THEN now() + make_interval(days => $3::int) \
                  ELSE now() + make_interval(days => m.duration_days) END \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(uid)
    .bind(medal_id)
    .bind(body.days.filter(|d| (1..=3650).contains(d)))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(d) = body.days {
        if !(1..=3650).contains(&d) {
            return Err(DomainError::Validation(
                "days 需在 1-3650 之间（或不传随勋章定义）".into(),
            ));
        }
    }
    state
        .repo
        .audit(
            Some(auth.id),
            "user.grant_medal",
            Some(uid),
        )
        .await;
    // 附送 PM（0286 P3a）：与批量发放/道具单发对齐
    let mname: String = sqlx::query_scalar(
        "SELECT name FROM medals WHERE id = $1",
    )
    .bind(medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    admin_pm(
        &state.repo.db,
        auth.id,
        uid,
        "管理员向你颁发了勋章",
        &format!("你获得了勋章「{mname}」，请到个人中心查看佩戴。"),
    )
    .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "medal_id": medal_id }),
    ))
}

#[derive(serde::Deserialize)]
pub(super) struct GrantMedalReq {
    /// 有效期覆盖（天）：缺省随勋章定义；1-3650
    #[serde(default)]
    days: Option<i32>,
}
/// 管理动作附送站内信（0286 P3a）：尽力而为，失败不影响发放结果。
async fn admin_pm(
    db: &sqlx::PgPool,
    sender: i64,
    uid: i64,
    subject: &str,
    body: &str,
) {
    let sql = "INSERT INTO messages \
               (sender_id, receiver_id, subject, body) \
               VALUES ($1, $2, $3, $4)";
    let _ = sqlx::query(sql)
        .bind(sender)
        .bind(uid)
        .bind(subject)
        .bind(body)
        .execute(db)
        .await;
}

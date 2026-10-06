//! 单发勋章（0286 从 user_detail.rs 拆出守 300 行门禁）：
//! 有效期覆盖（days）+ 附送 PM。0291 补三条实测口径：
//! - `days` **先校验再写**：旧顺序是「按勋章缺省期插入 → 再判 days 越界报 400」，
//!   接口报错而勋章已经发出，重试又被 `ON CONFLICT DO NOTHING` 吞掉，
//!   于是有效期永远停在缺省值上，而站长看到的是「这次发放失败了」。
//! - 权限与批量侧对齐（`medal.manage`）：此前只卡 staff.panel（90+），
//!   版主能给下级任意发勋章，而批量发同一枚勋章要 93+。
//! - 已持有者要**明说**：响应带 already_held 与本次实际到期时间，
//!   PM 只在真发出去时发（重复发放不该再打扰用户一次）。

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
    // 校验一律在写之前（见模块头第一条）
    if let Some(d) = body.days {
        if !(1..=3650).contains(&d) {
            return Err(DomainError::Validation(
                "days 需在 1-3650 之间（或不传随勋章定义）".into(),
            ));
        }
    }
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let (uid, medal_id) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM medals WHERE id = $1)")
            .bind(medal_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(medal_id));
    }
    // 有效期覆盖（0286 P3b）：缺省随勋章定义 duration_days（NULL=永久）；
    // 显式传 days（1-3650）则本次发放按覆盖值——活动限时勋章不再需要
    // 改勋章定义（改定义影响全体持有者的语义边界）
    let inserted = sqlx::query(
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
    .bind(body.days)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    // 回读实际到期时间：站长要核对的是「发出去那张卡在什么时候失效」
    let expires_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT expires_at FROM user_medals \
         WHERE user_id = $1 AND medal_id = $2",
    )
    .bind(uid)
    .bind(medal_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 发放台账（0291）：单发与批量同写一张表，一处对账
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let mut ledger = "ok";
    let spec = crate::admin_p3_http::BatchSpec {
        batch_id: &batch_id,
        idem: None,
        actor_id: auth.id,
        kind: "medal",
        amount: 1,
        item_id: None,
        medal_id: Some(medal_id),
        days: body.days,
        selector: serde_json::json!({ "mode": "single" }),
        targets: &[uid],
        subject: None,
    };
    if crate::admin_p3_http::batch_open(&state.repo.db, &spec)
        .await
        .is_err()
    {
        ledger = "write_failed";
    } else {
        let _ = crate::admin_p3_http::batch_finish(
            &state.repo.db,
            &batch_id,
            inserted as i64,
            "done",
            None,
        )
        .await;
    }

    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.grant_medal",
            Some(uid),
            None,
            Some(serde_json::json!({
                "medal_id": medal_id, "days": body.days,
                "already_held": inserted == 0, "batch_id": batch_id,
            })),
        )
        .await;
    // 附送 PM（0286 P3a）：只在真发出去的那一次发，重复发放不再打扰用户
    if inserted > 0 {
        let mname: String =
            sqlx::query_scalar("SELECT name FROM medals WHERE id = $1")
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
    }
    Ok(ok(serde_json::json!({
        "user_id": uid, "medal_id": medal_id,
        "already_held": inserted == 0, "expires_at": expires_at,
        "batch_id": batch_id, "ledger": ledger,
    })))
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

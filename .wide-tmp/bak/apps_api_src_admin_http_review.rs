use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

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
    // U2 §12.5 审核结果通知：发布者站内信必达（+邮件尽力），被拒附理由。
    {
        let row: Option<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT t.owner_id, t.title, d.label, t.deny_note FROM torrents t \
             LEFT JOIN torrent_deny_reasons d ON d.id = t.deny_reason_id WHERE t.id = $1",
        )
        .bind(body.torrent_id)
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten();
        if let Some((owner, title, deny_label, deny_note)) = row {
            let email: Option<String> = sqlx::query_scalar(
                "SELECT email FROM users WHERE id = $1 AND email <> ''",
            )
            .bind(owner)
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten();
            let (subject, body_text) = if body.approve {
                (
                    format!("种子过审：{title}"),
                    format!(
                        "你发布的种子已通过审核：#{id} {title}。",
                        id = body.torrent_id
                    ),
                )
            } else {
                let why = deny_label.or(deny_note).unwrap_or_else(|| {
                    let r = body.reason.trim();
                    if r.is_empty() {
                        "未注明".into()
                    } else {
                        r.to_string()
                    }
                });
                (
                    format!("种子被拒：{title}"),
                    format!(
                        "你发布的种子未通过审核：#{id} {title}\n原因：{why}\n可修改后重新发布。",
                        id = body.torrent_id
                    ),
                )
            };
            crate::mailer::notify(
                &state.repo.db,
                owner,
                email,
                &subject,
                &body_text,
            )
            .await;
        }
    }
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

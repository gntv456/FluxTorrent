//! 申诉（0020）：提交/我的/队列/处理。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 申诉 ============

mod create;
pub(crate) use create::appeal_create;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AppealRow {
    id: i64,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/appeals")]
pub async fn appeal_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<AppealRow> = sqlx::query_as(
        "SELECT id, kind, ref_id, body, status, result_note, created_at \
         FROM appeals WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealHandleReq {
    appeal_id: i64,
    accept: bool,
    note: String,
    /// 受理申诉后自动解封（0209 P1-8；缺省 true——受理即恢复，免两跳）
    #[serde(default = "yes_default")]
    unban: bool,
}

fn yes_default() -> bool {
    true
}

/// staff 侧申诉队列（open 优先，可按状态过滤）
#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminAppealRow {
    id: i64,
    username: String,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/appeals")]
pub async fn appeal_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<AppealQueueQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::APPEAL_HANDLE,
    )
    .await?;
    let status = q.status.as_deref().unwrap_or("");
    let rows: Vec<AdminAppealRow> = sqlx::query_as(
        "SELECT a.id, u.username, a.kind, a.ref_id, a.body, a.status, a.result_note, a.created_at \
         FROM appeals a JOIN users u ON u.id = a.user_id \
         WHERE ($1 = '' OR a.status = $1) \
         ORDER BY (a.status = 'open') DESC, a.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealQueueQuery {
    status: Option<String>,
}

#[post("/admin/appeals/handle")]
pub async fn appeal_handle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealHandleReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::APPEAL_HANDLE)
        .await?;
    // P-634（P-466 同族）：这枚 note 同时写进 appeals.result_note、当事人
    // 收到的站内信正文与邮件正文，而界面那条输入框连 maxLength 都没有——
    // 私信写口自己有 5000 字上限，站方这条路却能把任意长正文从旁边灌进
    // 同一个收件箱。上限与所答复的申诉正文同一枚数（2000），装在查库之前。
    if body.note.trim().chars().count() > 2000 {
        return Err(DomainError::Validation("处理意见最长 2000 字".into()));
    }
    // 0209 P1-8：受理即联动解封（可选）+ 双通道通知当事人（此前 accept 只改 appeals 表，
    // 站长须手动恢复 status=0，且被封者不能登录看 PM、收不到任何处理结果）。
    // 0285 解封先于翻转；只认 open+2，墓碑(3)不可复活。
    // 批七轮补同一口径的第二段：kind='hr' 的受理过去只翻状态、发一句「已受理」，
    // 会员照旧被 hr_flag 挡在下载口外——受理与处置是两回事，却写成了一回事。
    let (user_id, email, kind, hr_target, mut unbanned): (
        i64,
        Option<String>,
        String,
        Option<i64>,
        bool,
    ) = sqlx::query_as(
        "SELECT a.user_id, u.email, a.kind, a.ref_id, \
         ($2 AND a.kind = 'ban' AND a.status = 'open' \
          AND u.status = 2) \
         FROM appeals a JOIN users u ON u.id = a.user_id WHERE a.id = $1",
    )
    .bind(body.appeal_id)
    .bind(body.accept && body.unban)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or((0, None, String::new(), None, false));
    if unbanned {
        // 0285：解封需严格高于目标（旧版任何 staff 都能放出被封的管理员/站长）
        unbanned = super::appeal_unban::restore_if_outranked(
            &state,
            auth.id,
            auth.class_id,
            user_id,
        )
        .await?;
    }
    // 与解封同一口径（也同一失败面）：等级不够赦不得这条时，整个受理一起失败，
    // 申诉留在 open，不会出现「回了已受理、记录还在违规」。
    let mut hr_pardoned = false;
    if body.accept && kind == "hr" {
        if let Some(tid) = hr_target {
            let reason = if body.note.trim().is_empty() {
                "申诉受理：撤销该条 H&R 违规"
            } else {
                body.note.trim()
            };
            hr_pardoned = crate::services::hr_pardon::pardon_one(
                &state,
                auth.id,
                auth.class_id,
                "hr.pardon",
                user_id,
                tid,
                reason,
            )
            .await?;
        }
    }
    let n = sqlx::query(
        "UPDATE appeals SET status = $2, handled_by = $1, handled_at = now(), result_note = $3 \
         WHERE id = $4 AND status = 'open'",
    )
    .bind(auth.id)
    .bind(if body.accept { "accepted" } else { "rejected" })
    .bind(body.note.trim())
    .bind(body.appeal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("申诉不存在或已处理".into()));
    }
    if user_id > 0 {
        let site: String = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'site_name'), 'FluxTorrent')",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "FluxTorrent".into());
        let verdict = if body.accept {
            "已受理"
        } else {
            "未通过"
        };
        let subject = format!("[{site}] 你的申诉{verdict}");
        // 回执必须说清真做了什么事：只回「已受理」而记录还在违规，
        // 会员会以为限制已解除，下一次下载才撞墙。
        let tail = if unbanned {
            "账号已恢复，现在可以正常登录。"
        } else if kind == "hr" && body.accept && hr_pardoned {
            "对应那条 H&R 违规已一并撤销，下载限制已解除。"
        } else if kind == "hr" && body.accept {
            "本次受理没有附带可撤销的 H&R 记录：申诉未指明条目，或该条已不是\
             「已违规」状态。如需撤销该条违规，请到「我的 H&R」按条目重新申诉，\
             或联系职员在 H&R 管理里处理。"
        } else {
            ""
        };
        let body_text = format!(
            "你的申诉（#{id}）处理结果：{verdict}。\n处理说明：{note}\n{tail}",
            id = body.appeal_id,
            note = if body.note.trim().is_empty() {
                "（无）"
            } else {
                body.note.trim()
            },
        );
        // 被拒者可登录看 PM；仍被封者至少有邮件（未配 SMTP 则只留 PM）
        crate::mailer::notify(
            &state.repo.db,
            user_id,
            email,
            &subject,
            &body_text,
        )
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "appeal.handle", Some(body.appeal_id))
        .await;
    Ok(ok(serde_json::json!({
        "handled": body.appeal_id,
        "unbanned": unbanned,
        "hr_pardoned": hr_pardoned,
    })))
}

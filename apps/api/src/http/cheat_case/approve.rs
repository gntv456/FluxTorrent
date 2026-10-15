//! 高危处置批准（0369 M3 四眼）：POST /admin/cheat-cases/{id}/approve。
//! 从 cheat_case/mod.rs 拆出（300 行门禁）。
//!
//! resolve 对 ban/full_ban 只落 pending（决定人 ≠ 批准人）：本端点由**另一名**
//! 有 cheat.sanctions.manage 的 staff 批准后生效。批准即：
//!   ① users.cheat_banned_until = now() + ban_days（full_ban 另置 status=2）
//!   ② 签发一次性申诉令牌（32B 随机 → sha3-256 存 users.appeal_token_hash，
//!      明文只出现在发给用户的站内信/邮件里；令牌形如 {case_id}.{hex}）
//!   ③ sanction.state: pending → applied（记录批准人），案件关案
//! 停权的实际约束：seeding.rs 停发做种收益 + ensure_downloadable 禁下载；
//! 到期由 worker cheat_enforce 每 10m 清扫（users.cheat_banned_until < now）。
//! full_ban 的 status=2 不随停权到期自动恢复（手动/申诉 overturned）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::super::auth_infra::require_auth;
use crate::admin_http::guard::ensure_outranks;
use crate::authz::perm::CHEAT_SANCTIONS;
use crate::authz::require_perm;
use crate::bencode::hex;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
struct ApproveReq {
    /// 批准意见（进案件备注；可空）
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/cheat-cases/{id}/approve")]
pub async fn cheat_case_approve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ApproveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_perm(&state, &auth, CHEAT_SANCTIONS).await?;
    let id = path.into_inner();
    let case: Option<(i64, i16, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT ref_id, status, sanction FROM reports \
         WHERE id = $1 AND ref_type = 'cheat'",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, _status, sj)) = case else {
        return Err(DomainError::NotFound(id));
    };
    let sj = sj.ok_or_else(|| {
        DomainError::Validation("该案件没有待批准的处置".into())
    })?;
    let st = sj.get("state").and_then(|v| v.as_str());
    let is_pending = st == Some("pending");
    let kind = sj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if !is_pending || !matches!(kind, "ban" | "full_ban") {
        return Err(DomainError::Validation(
            "该案件没有待批准的高危处置（ban/full_ban）".into(),
        ));
    }
    // 四眼：批准人不能是决定人本人（决定人已表达过一次判断）
    let decider = sj.get("decider").and_then(|v| v.as_i64()).unwrap_or(0);
    if decider == auth.id {
        return Err(DomainError::Validation(
            "决定人与批准人不能相同（四眼原则）".into(),
        ));
    }
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let ban_days = sj.get("ban_days").and_then(|v| v.as_i64()).unwrap_or(30);
    let note = body.note.as_deref().unwrap_or("").trim();

    let mut tx = state.repo.db.begin().await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // ① 停权生效（full_ban 叠加全站封禁 status=2）
    sqlx::query(
        "UPDATE users SET \
           cheat_banned_until = now() + make_interval(days => $1::int) \
         WHERE id = $2",
    )
    .bind(ban_days)
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if kind == "full_ban" {
        sqlx::query("UPDATE users SET status = 2 WHERE id = $1")
            .bind(uid)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // ② sanction: pending → applied + 关案
    let applied = serde_json::json!({
        "kind": kind,
        "state": "applied",
        "decider": decider,
        "approver": auth.id,
        "at": sj.get("at").cloned(),
        "applied_at": chrono::Utc::now(),
        "warn_weeks": sj.get("warn_weeks").cloned(),
        "ban_days": ban_days,
        "reclaim_up": sj.get("reclaim_up").cloned(),
        "note": sj.get("note").cloned(),
    });
    sqlx::query(
        "UPDATE reports SET status = 1, handled_by = $1, \
         handled_at = now(), sanction = $2 WHERE id = $3",
    )
    .bind(auth.id)
    .bind(&applied)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // ③ 案件备注（批准台账）
    let label = if kind == "full_ban" {
        format!("全站封禁已批准生效（停权记录 {ban_days} 天）")
    } else {
        format!("作弊停权 {ban_days} 天已批准生效")
    };
    sqlx::query(
        "INSERT INTO case_notes (case_id, author_id, body) \
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(auth.id)
    .bind(format!(
        "批准：{label}；批准人 ≠ 决定人 #{decider}。{}",
        if note.is_empty() { "（无批准意见）" } else { note }
    ))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // ④ 一次性申诉令牌：{case_id}.{48 hex}；只存 sha3-256，明文随通知发出
    let raw: [u8; 24] = rand::random();
    let token = format!("{id}.{}", hex(&raw));
    use sha3::Digest;
    let mut h = sha3::Sha3_256::new();
    h.update(token.as_bytes());
    let d = h.finalize();
    let token_hash: String = d.iter().map(|b| format!("{b:02x}")).collect();
    let token_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'appeal_token_days'), 30)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(30);
    sqlx::query(
        "UPDATE users SET appeal_token_hash = $1, \
           appeal_token_expires_at = now() + make_interval(days => $2::int) \
         WHERE id = $3",
    )
    .bind(&token_hash)
    .bind(token_days)
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // ⑤ 审计 + 通知（令牌明文只在此出现）
    state
        .repo
        .audit(Some(auth.id), "cheat_case.approve", Some(id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        uid,
        Some(auth.id),
        &format!("高危处置批准：{label}（案件 #{id}）"),
    )
    .await;
    let target: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT username, email FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten();
    if let Some((username, email)) = target {
        let site: String = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name = 'site_name'), 'FluxTorrent')",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "FluxTorrent".into());
        crate::mailer::notify(
            &state.repo.db,
            uid,
            email,
            &format!("[{site}] 账号处罚通知"),
            &format!(
                "你好 {username}，管理组已对你的账号执行：{label}。\n\
                 如对处置有异议，可在 {token_days} 天内凭以下申诉令牌在\
                 申诉页提交复核（一次性有效，请勿外传）：\n{token}\n\
                 令牌遗失或过期将无法通过本通道申诉。",
            ),
        )
        .await;
    }
    Ok(ok(serde_json::json!({
        "id": id,
        "approved": true,
        "kind": kind,
        "ban_days": ban_days,
    })))
}

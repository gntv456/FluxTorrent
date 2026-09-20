//! H&R 追责（0020）：我的 H&R / 赦免（单发+批量+自救）。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ H&R 追责 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct HrRow {
    torrent_id: i64,
    torrent_name: String,
    required_seconds: i32,
    seeded_seconds: i32,
    deadline: chrono::DateTime<chrono::Utc>,
    status: String,
}

#[get("/me/hr")]
pub async fn my_hr_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<HrRow> = sqlx::query_as(
        "SELECT h.torrent_id, t.name AS torrent_name, h.required_seconds, \
                h.seeded_seconds, h.deadline, h.status \
         FROM hr_snapshots h JOIN torrents t ON t.id = h.torrent_id \
         WHERE h.user_id = $1 ORDER BY h.deadline LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct PardonReq {
    user_id: i64,
    torrent_id: i64,
    note: String,
}

/// H&R 赦免（staff）：违规 → pardoned，violation 标记 resolved
#[post("/admin/hr/pardon")]
pub async fn hr_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PardonReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON)
        .await?;
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("赦免必须填理由".into()));
    }
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now() \
         WHERE user_id = $2 AND torrent_id = $3 AND status = 'violated'",
    )
    .bind(auth.id)
    .bind(body.user_id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("无待赦免的 H&R 违规".into()));
    }
    // 审计修复（P1）：赦免后 snatches.hr_flag 不回清（worker 只会置 TRUE，全库无 FALSE
    // 路径），列表/详情的 H&R 角标在赦免后仍然残留。此处同步回清（与快照口径一致）。
    let _ =
        sqlx::query("UPDATE snatches SET hr_flag = FALSE WHERE user_id = $1 AND torrent_id = $2")
            .bind(body.user_id)
            .bind(body.torrent_id)
            .execute(&state.repo.db)
            .await;
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
    )
    .bind(auth.id)
    .bind(body.user_id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "hr.pardon", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "pardoned": true })))
}

#[derive(Deserialize)]
struct BatchPardonReq {
    /// 批量豁免目标：(user_id, torrent_id) 对列表；空数组报错
    items: Vec<BatchPardonItem>,
    note: String,
}

#[derive(Deserialize)]
struct BatchPardonItem {
    user_id: i64,
    torrent_id: i64,
}

/// H&R 批量赦免（staff）：一次豁免多条违规（NP postmanage 批量口径）。
/// 单条失败不回滚其他（每对独立 UPDATE），返回成功/跳过明细。
#[post("/admin/hr/pardon/batch")]
pub async fn hr_pardon_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BatchPardonReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON)
        .await?;
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("赦免必须填理由".into()));
    }
    if body.items.is_empty() || body.items.len() > 500 {
        return Err(DomainError::Validation(
            "批量豁免条目须在 1~500 之间".into(),
        ));
    }
    let mut pardoned: Vec<serde_json::Value> = Vec::new();
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    for it in &body.items {
        let n = sqlx::query(
            "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now()              WHERE user_id = $2 AND torrent_id = $3 AND status = 'violated'",
        )
        .bind(auth.id)
        .bind(it.user_id)
        .bind(it.torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n == 0 {
            skipped.push(serde_json::json!({ "user_id": it.user_id, "torrent_id": it.torrent_id }));
            continue;
        }
        sqlx::query(
            "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1              WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
        )
        .bind(auth.id)
        .bind(it.user_id)
        .bind(it.torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        pardoned.push(serde_json::json!({ "user_id": it.user_id, "torrent_id": it.torrent_id }));
    }
    state
        .repo
        .audit(Some(auth.id), "hr.pardon_batch", None)
        .await;
    Ok(ok(serde_json::json!({
        "pardoned": pardoned, "skipped": skipped,
        "pardoned_count": pardoned.len(), "skipped_count": skipped.len(),
    })))
}

#[derive(Deserialize)]
struct SelfPardonReq {
    torrent_id: i64,
}

/// 自助免罪（B-02）：消耗 20000 火花，赦免自己一条 violated H&R（NP 魔力免罪口径）
#[post("/me/hr/pardon")]
pub async fn hr_self_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SelfPardonReq>,
) -> DomainResult<HttpResponse> {
    const SELF_PARDON_PRICE: i64 = 20_000;
    let auth = require_auth(&req, &state).await?;
    // 先扣火花，成功后才赦免 —— 原顺序（先 UPDATE 后扣费）在余额不足时会把违规白白
    // 赦免（扣费失败仅回 4001，状态已不可逆），且两步非事务，中途失败同样撕裂。
    // spend_spark 自带行锁 + 幂等键，先扣可保证「未付费必不赦免」。
    let idem = format!("hr-self-pardon:{}:{}", auth.id, body.torrent_id);
    let outcome = crate::economy_http::spend_spark(
        &state.repo.db,
        auth.id,
        SELF_PARDON_PRICE,
        "hr_pardon",
        &idem,
        "hr",
        body.torrent_id,
    )
    .await?;
    // 审计修复（P0 铸币）：Replayed = 本请求未扣款（幂等键命中的是历史成功扣费）。
    // 旧逻辑忽略该返回值继续走赦免/退款分支，退款键又拼随机 UUID 每次全新，
    // 重放请求可无限净赚 20000/次。现在：重放一律拒绝，退款键改为确定性键。
    if !matches!(outcome, crate::economy_http::SpendOutcome::Spent) {
        return Err(DomainError::Validation(
            "该违规已处理过，请勿重复提交".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now() \
         WHERE user_id = $1 AND torrent_id = $2 AND status = 'violated' \
         RETURNING user_id",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.is_none() {
        // 并发窗口内另一请求已赦免（或本就不存在）：退回本次扣费，避免花 20000 买空气。
        // 退款键去掉随机 UUID：同一 (user, torrent) 的退款与扣款一对一，重放不产生新流水。
        crate::economy_http::earn_spark(
            &state.repo.db,
            auth.id,
            SELF_PARDON_PRICE,
            "hr_pardon_refund",
            &format!("{idem}:refund"),
        )
        .await?;
        return Err(DomainError::Validation("无待免罪的 H&R 违规".into()));
    }
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $1 AND torrent_id = $2 AND resolved_at IS NULL",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "hr.self_pardon", Some(body.torrent_id))
        .await;
    Ok(ok(
        serde_json::json!({ "pardoned": body.torrent_id, "cost": SELF_PARDON_PRICE }),
    ))
}

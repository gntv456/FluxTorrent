//! 反作弊案件管理 M1（0367，对标 PT Studio anticheat 运营层）：
//! cheat_events 扁平板账 → 可协同的案件。策略「长在」：案件是 reports 表
//! ref_type='cheat' 的行（复用 0 待处理/2 已认领/1 已处理状态机与认领互斥），
//! 证据归案写 cheat_events.case_id，协同记录走 case_notes。
//!
//! 与既有 cheat-events 端点的关系：那个管「单条证据的处置勾销」，
//! 本模块管「针对一个用户的调查案件」——先开案聚合证据，再在案件下协同。

mod approve;
mod detail;
mod list;
mod mount;
mod sanction;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::auth_infra::{require_auth, require_staff};
use crate::admin_http::guard::ensure_outranks;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

pub use approve::cheat_case_approve;
pub use detail::cheat_case_detail;
pub use list::cheat_case_list;
pub use mount::mount_cheat_cases;
pub use sanction::cheat_case_resolve;

/// 复核 SLA：超过此时长未首次处理标 sla_breached（worker 置位）。
const SLA_HOURS: i64 = 72;

#[derive(Deserialize)]
struct OpenCaseReq {
    /// 从一条证据行开案（取其 user 为对象）；与 user_id 二选一
    #[serde(default)]
    event_id: Option<i64>,
    /// 直接从用户开案（聚合其全部未处置证据）
    #[serde(default)]
    user_id: Option<i64>,
    /// 0 普通 / 1 加急 / 2 危急
    #[serde(default)]
    priority: Option<i64>,
    /// 开案原因（首条备注 + reports.reason）
    #[serde(default)]
    note: Option<String>,
}

/// POST /admin/cheat-cases/open：开案（从证据行或用户）。
/// 同用户至多一个进行中案件（uq_cheat_case_open）；已存在返回 duplicate。
#[post("/admin/cheat-cases/open")]
pub async fn cheat_case_open(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OpenCaseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let (event_id, user_id) = match (body.event_id, body.user_id) {
        (Some(e), _) => {
            let uid: Option<i64> = sqlx::query_scalar(
                "SELECT user_id FROM cheat_events WHERE id = $1",
            )
            .bind(e)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            (Some(e), uid.ok_or(DomainError::NotFound(e))?)
        }
        (None, Some(u)) => (None, u),
        (None, None) => {
            return Err(DomainError::Validation(
                "event_id 与 user_id 至少给一个".into(),
            ))
        }
    };
    // 护栏：不能调查自己/上级（与赦免、改档同族；M3 的处置再加四眼）
    ensure_outranks(&state.repo.db, auth.class_id, user_id).await?;
    let priority = body.priority.unwrap_or(0).clamp(0, 2);
    let note = body
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    // 摘要 = 该用户未处置证据的画像分布（开案理由的客观依据）
    // 注意 sum(hits) 在 PG 返回 NUMERIC（hits 是 bigint），必须 ::bigint
    // 强转后才能按 Rust i64 解码（冒烟第 1 轮 500 的根因）。
    let agents: Vec<(String, i64)> = sqlx::query_as(
        "SELECT agent, COALESCE(sum(hits), 0)::bigint FROM cheat_events \
         WHERE user_id = $1 AND resolved_at IS NULL \
         GROUP BY agent ORDER BY 2 DESC",
    )
    .bind(user_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let summary = agents
        .iter()
        .map(|(a, h)| format!("{a}×{h}"))
        .collect::<Vec<_>>()
        .join("，");
    let reason = format!(
        "{}（{} 类未处置证据）",
        note.unwrap_or("反作弊调查"),
        agents.len()
    );
    // 开案：ON CONFLICT 命中 = 已有进行中案件 → duplicate 不新开。
    // risk_score 取当前风险分快照（0368 M2，与 worker auto_open_case 同口径；
    // 手动开案多发生在观察名单巡逻后，快照让队列排序有据可查）。
    let case_id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO reports \
           (reporter_id, ref_type, ref_id, reason, priority, risk_score) \
         VALUES ($1, 'cheat', $2, $3, $4, \
           (SELECT score FROM user_risk_scores WHERE user_id = $2)) \
         ON CONFLICT (ref_id) WHERE ref_type = 'cheat' AND status IN (0, 2) \
         DO NOTHING RETURNING id",
    )
    .bind(auth.id)
    .bind(user_id)
    .bind(&reason)
    .bind(priority as i16)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(case_id) = case_id else {
        return Ok(ok(serde_json::json!({
            "duplicate": true,
            "hint": "该用户已有进行中案件（同用户至多一个）",
        })));
    };
    // 证据归案：该用户全部游离未处置证据挂到本案（指定 event_id 时同路径，
    // 事件必属该用户；游离= case_id IS NULL）
    let n = sqlx::query(
        "UPDATE cheat_events SET case_id = $1 \
         WHERE user_id = $2 AND resolved_at IS NULL AND case_id IS NULL",
    )
    .bind(case_id)
    .bind(user_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    // 首条备注（开案理由 + 归案证据数）
    sqlx::query(
        "INSERT INTO case_notes (case_id, author_id, body) VALUES ($1, $2, $3)",
    )
    .bind(case_id)
    .bind(auth.id)
    .bind(format!("开案：{reason}；归案未处置证据 {n} 条。{summary}"))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "cheat_case.open",
            Some(case_id),
            note,
            Some(serde_json::json!({
                "target_user": user_id,
                "event_id": event_id,
                "evidence_attached": n,
            })),
        )
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        user_id,
        Some(auth.id),
        &format!("开立反作弊案件 #{case_id}（归案证据 {n} 条）"),
    )
    .await;
    Ok(ok(serde_json::json!({
        "id": case_id,
        "duplicate": false,
        "user_id": user_id,
        "evidence_attached": n,
    })))
}

#[derive(Deserialize)]
struct ClaimReq {
    /// true=认领（互斥）；false=释放回队列
    #[serde(default)]
    release: Option<bool>,
}

/// POST /admin/cheat-cases/{id}/claim：认领互斥 / 释放。
/// 认领语义与 admin_http/reports.rs 的工单认领同口径（0285）。
#[post("/admin/cheat-cases/{id}/claim")]
pub async fn cheat_case_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let id = path.into_inner();
    let release = body.release.unwrap_or(false);
    let sql = if release {
        "UPDATE reports SET status = 0, claimed_by = NULL, claimed_at = NULL \
         WHERE id = $1 AND ref_type = 'cheat' AND status = 2 \
           AND claimed_by = $2"
    } else {
        "UPDATE reports SET status = 2, claimed_by = $2, claimed_at = now() \
         WHERE id = $1 AND ref_type = 'cheat' AND status IN (0, 2) \
           AND (claimed_by IS NULL OR claimed_by = $2)"
    };
    let n = sqlx::query(sql)
        .bind(id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM reports \
             WHERE id = $1 AND ref_type = 'cheat')",
        )
        .bind(id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !exists {
            return Err(DomainError::NotFound(id));
        }
        return Err(DomainError::Validation(
            "案件已被他人认领或状态不允许（并发互斥）".into(),
        ));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            if release {
                "cheat_case.release"
            } else {
                "cheat_case.claim"
            },
            Some(id),
            None,
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "id": id, "released": release, "claimed": !release,
    })))
}

#[derive(Deserialize)]
struct CaseNoteReq {
    body: String,
}

/// POST /admin/cheat-cases/{id}/note：协同备注（处理过程记录）。
#[post("/admin/cheat-cases/{id}/note")]
pub async fn cheat_case_note(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CaseNoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let id = path.into_inner();
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 4000 {
        return Err(DomainError::Validation("备注 1-4000 字符".into()));
    }
    let n: i64 = sqlx::query_scalar(
        "INSERT INTO case_notes (case_id, author_id, body) \
         SELECT $1, $2, $3 WHERE EXISTS( \
           SELECT 1 FROM reports WHERE id = $1 AND ref_type = 'cheat') \
         RETURNING id",
    )
    .bind(id)
    .bind(auth.id)
    .bind(text)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::NotFound(id))?;
    Ok(ok(serde_json::json!({ "id": n, "case_id": id })))
}

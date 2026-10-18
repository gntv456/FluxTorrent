//! 案件处置（0369 M3）：POST /admin/cheat-cases/{id}/resolve。
//! 从 cheat_case/mod.rs 拆出（300 行门禁）。
//!
//! 处置选项（sanction）：none / warn / ban / full_ban，外加两个正交开关：
//!   resolve_evidence——对本案证据置 resolved_at（即时解除做种收益停发，
//!     seeding.rs 的 NOT EXISTS 口径）；evidence_ids 可只处置选定证据。
//!   reclaim_up——回收虚报上传量：traffic_ledger 负向流水（operator_id=
//!     操作人、reason=案件号）+ 同步 users.uploaded 快照列。申诉 overturned
//!     时由 cheat_appeals 反向补回（补上 M0 盘点的「held 量无补发接口」）。
//! 四眼：ban/full_ban 是高危处置，本端点只落 **pending**（reports.sanction
//! JSONB state=pending），由另一名有 sanctions 权限的 staff 经
//! /admin/cheat-cases/{id}/approve 批准后才生效——决定人 ≠ 批准人。
//! warn 为 Gazelle 顺延语义：从现有到期时刻起算，不覆盖未来时点。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::super::auth_infra::require_auth;
use crate::admin_http::guard::ensure_outranks;
use crate::authz::perm::CHEAT_SANCTIONS;
use crate::authz::require_perm;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
struct ResolveReq {
    /// none（缺省）/ warn / ban / full_ban；后两者进四眼待批准
    #[serde(default)]
    sanction: Option<String>,
    /// sanction=warn：警告周数（1-52，缺省 2）
    #[serde(default)]
    warn_weeks: Option<i32>,
    /// sanction=ban：停权天数（缺省取 settings cheat_ban_default_days）
    #[serde(default)]
    ban_days: Option<i32>,
    /// 回收虚报上传量（字节；0/缺省 = 不回收）
    #[serde(default)]
    reclaim_up: Option<i64>,
    /// 是否对本案证据置 resolved_at（缺省 true——结案即解除停发）
    #[serde(default)]
    resolve_evidence: Option<bool>,
    /// 只处置选定证据（缺省 = 本案全部未处置证据）
    #[serde(default)]
    evidence_ids: Option<Vec<i64>>,
    /// 是否关案（status=1；pending 四眼时强制留开，缺省 true）
    #[serde(default)]
    close: Option<bool>,
    /// 处置说明（进案件备注 + 通知用户）
    note: String,
}

#[post("/admin/cheat-cases/{id}/resolve")]
pub async fn cheat_case_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_perm(&state, &auth, CHEAT_SANCTIONS).await?;
    let id = path.into_inner();
    let case: Option<(i64, i16)> = sqlx::query_as(
        "SELECT ref_id, status FROM reports \
         WHERE id = $1 AND ref_type = 'cheat'",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, st)) = case else {
        return Err(DomainError::NotFound(id));
    };
    if st == 1 {
        return Err(DomainError::Validation("案件已处理，不能重复处置".into()));
    }
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;

    let kind = body.sanction.as_deref().unwrap_or("none");
    if !["none", "warn", "ban", "full_ban"].contains(&kind) {
        return Err(DomainError::Validation(
            "sanction 只认 none/warn/ban/full_ban".into(),
        ));
    }
    let note = body.note.trim();
    if note.is_empty() || note.chars().count() > 4000 {
        return Err(DomainError::Validation("处置说明 1-4000 字符".into()));
    }
    let warn_weeks = body.warn_weeks.unwrap_or(2).clamp(1, 52);
    let reclaim_up = body.reclaim_up.unwrap_or(0).max(0);
    let ban_days: i32 = match body.ban_days {
        Some(d) => d.clamp(1, 3650),
        None => sqlx::query_scalar(
            "SELECT COALESCE((SELECT value::bigint FROM site_settings \
             WHERE name = 'cheat_ban_default_days'), 30)",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(30) as i32,
    };
    let resolve_ev = body.resolve_evidence.unwrap_or(true);
    // 高危处置（封禁类）走四眼：本端点只记 pending，不落用户侧变更
    let pending = matches!(kind, "ban" | "full_ban");
    let close = body.close.unwrap_or(true) && !pending;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // ① 证据处置（即时解除做种收益停发；seeding.rs NOT EXISTS 口径）
    let n_ev: i64 = if resolve_ev {
        let r = match body.evidence_ids.as_ref().filter(|v| !v.is_empty()) {
            Some(ids) => sqlx::query(
                "UPDATE cheat_events SET resolved_at = now() \
                 WHERE case_id = $1 AND id = ANY($2) AND resolved_at IS NULL",
            )
            .bind(id)
            .bind(ids)
            .execute(&mut *tx)
            .await
            .map(|r| r.rows_affected()),
            None => sqlx::query(
                "UPDATE cheat_events SET resolved_at = now() \
                 WHERE case_id = $1 AND resolved_at IS NULL",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map(|r| r.rows_affected()),
        };
        r.map_err(|e| DomainError::Internal(e.into()))? as i64
    } else {
        0
    };
    // ② warn（Gazelle 顺延：从现有到期时刻与 now 的较晚者起算）
    if kind == "warn" {
        sqlx::query(
            "UPDATE users SET \
               warned_until = GREATEST(COALESCE(warned_until, now()), now()) \
                 + make_interval(weeks => $1::int), \
               warned_reason = $2 \
             WHERE id = $3",
        )
        .bind(warn_weeks)
        .bind(format!("反作弊案件 #{id}：{note}"))
        .bind(uid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // ③ 回收虚报上传量（负向流水 + 同步快照列；申诉 overturned 时反向）
    if reclaim_up > 0 {
        sqlx::query(
            "INSERT INTO traffic_ledger \
               (id, user_id, torrent_id, delta_up, delta_down, \
                window_start, reason, operator_id) \
             VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, \
                     now(), $3, $4)",
        )
        .bind(uid)
        .bind(-reclaim_up)
        .bind(format!("cheat reclaim case #{id}"))
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "UPDATE users SET uploaded = GREATEST(uploaded - $1, 0) \
             WHERE id = $2",
        )
        .bind(reclaim_up)
        .bind(uid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // ④ sanction JSONB + 关案
    let sj = if kind == "none" && reclaim_up == 0 {
        serde_json::Value::Null
    } else {
        serde_json::json!({
            "kind": if kind == "none" { "reclaim" } else { kind },
            "state": if pending { "pending" } else { "applied" },
            "decider": auth.id,
            "at": chrono::Utc::now(),
            "warn_weeks": warn_weeks,
            "ban_days": ban_days,
            "reclaim_up": reclaim_up,
            "note": note,
        })
    };
    if close {
        sqlx::query(
            "UPDATE reports SET status = 1, handled_by = $1, \
             handled_at = now(), sanction = $2 WHERE id = $3",
        )
        .bind(auth.id)
        .bind(&sj)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else if !sj.is_null() {
        sqlx::query("UPDATE reports SET sanction = $1 WHERE id = $2")
            .bind(&sj)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // ⑤ 案件备注（处置台账）
    let kind_label = if kind == "none" {
        if reclaim_up > 0 {
            format!("回收上传 {} 字节", reclaim_up)
        } else {
            "无处置（结案）".to_string()
        }
    } else {
        match kind {
            "warn" => format!("警告 {warn_weeks} 周"),
            "ban" => format!("作弊停权 {ban_days} 天（待批准）"),
            _ => format!("全站封禁（待批准）"),
        }
    };
    sqlx::query(
        "INSERT INTO case_notes (case_id, author_id, body) \
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(auth.id)
    .bind(format!(
        "处置：{kind_label}；解除证据 {n_ev} 条。说明：{note}"
    ))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // ⑥ 审计 + 用户通知（warn 立即通知；ban/full_ban 在批准时带令牌通知）
    state
        .repo
        .audit(Some(auth.id), "cheat_case.resolve", Some(id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        uid,
        Some(auth.id),
        &format!("反作弊处置：{kind_label}（案件 #{id}）"),
    )
    .await;
    if kind == "warn" {
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
                &format!("[{site}] 你收到了一次警告（{warn_weeks} 周）"),
                &format!(
                    "你好 {username}，管理组对你的账号发出了为期 {warn_weeks} 周的警告。\n\
                     理由：{note}\n警告期间请遵守站点规则；如有疑问可提交申诉。",
                ),
            )
            .await;
        }
    }
    Ok(ok(serde_json::json!({
        "id": id,
        "closed": close,
        "pending": pending,
        "evidence_resolved": n_ev,
        "sanction": sj,
    })))
}

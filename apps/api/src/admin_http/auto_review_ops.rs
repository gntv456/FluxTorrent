//! 自动审种·管理台动作端点（0400 批 2，从 auto_review_admin.rs 按域拆出，
//! 300 行门禁）：回放预演 / 应用预设 / 按规则批量恢复，以及两者共用的
//! 设定写入助手。查询侧（stats/logs）留在 auto_review_admin.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 批量恢复的默认回溯天数（缺省 90）
fn default_days() -> i32 {
    90
}

#[derive(Deserialize)]
struct BulkRestoreReq {
    rule: String,
    #[serde(default = "default_days")]
    days: i32,
}

#[post("/admin/auto-review/replay")]
async fn replay(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    // 重算逻辑与 worker 同源：flux_shared::auto_review::assess。
    // 取近 90 天有人工终审的台账样本（回放的黄金集：human_verdict 已知）
    let db = &state.repo.db;
    let cfg = crate::admin_http::review_auto::engine_config_from_db(db).await;
    // count(*) 在 PG 里是 int8（INT8）：按 i32 解码必报「类型不匹配」
    // ⇒ 这个口恒 500（列 7/8 两处近 90 天计数）。改成 i64 再在下面收窄。
    let rows: Vec<(
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        i32,
        i32,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT t.id, t.owner_id, t.name, t.small_descr, t.descr, \
                    COALESCE(u.publish_credits, 0), \
                    COALESCE(u.approve_streak, 0), \
                    COALESCE((SELECT count(*) FROM torrents x \
                      WHERE x.owner_id = t.owner_id AND x.approval_status = 1 \
                        AND x.approved_at > now()
                          - make_interval(days => 90)), 0), \
                    COALESCE((SELECT count(*) FROM torrents x \
                      WHERE x.owner_id = t.owner_id AND x.approval_status = 2 \
                        AND x.mtime > now() \
                          - make_interval(days => 90)), 0) \
             FROM auto_review_log l \
             JOIN torrents t ON t.id = l.torrent_id \
             JOIN users u ON u.id = t.owner_id \
             WHERE l.decided_by = 0 AND t.approval_status IN (1, 2) \
             ORDER BY l.created_at DESC LIMIT 500",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut changed = 0i64;
    let mut same = 0i64;
    let mut samples: Vec<serde_json::Value> = Vec::new();
    for (tid, owner, name, small, descr, credits, streak, approved, denied) in
        rows
    {
        let snap = flux_shared::auto_review::TorrentSnapshot {
            torrent_id: tid,
            owner_id: owner,
            title: name.clone(),
            small_descr: small.unwrap_or_default(),
            descr: descr.unwrap_or_default(),
            category_id: 0,
            site_type: String::new(),
            file_names: vec![],
            recent_titles: vec![],
        };
        let profile = flux_shared::auto_review::PublisherProfile {
            publish_credits: credits,
            approve_streak: streak,
            registered_days: 0,
            approved_90d: approved as i32,
            denied_90d: denied as i32,
        };
        let a = flux_shared::auto_review::assess(&snap, &profile, &cfg);
        // 与台账原 verdict 比（0 过 1 拒 2 缓）
        let new_v = match a.suggestion {
            flux_shared::auto_review::Suggestion::Pass => 0,
            flux_shared::auto_review::Suggestion::Deny => 1,
            flux_shared::auto_review::Suggestion::Postpone => 2,
        };
        let old: i16 = sqlx::query_scalar(
            "SELECT verdict FROM auto_review_log \
             WHERE torrent_id = $1 AND decided_by = 0",
        )
        .bind(tid)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or(-1);
        if old == new_v {
            same += 1;
        } else {
            changed += 1;
            if samples.len() < 20 {
                samples.push(serde_json::json!({
                    "torrent_id": tid, "name": name,
                    "old_verdict": old, "new_verdict": new_v,
                    "new_score": a.score,
                }));
            }
        }
    }
    Ok(ok(serde_json::json!({
        "replayed": same + changed, "same": same, "changed": changed,
        "diff_samples": samples,
    })))
}

/// POST /admin/auto-review/apply：套用一键预设或站型出厂包。
/// 写 site_settings 走 UPSERT（设置保存链同款失效由 cfgver 轮询承接），
/// 并把选择记进 auto_review_preset / auto_review_site_preset 键。
#[post("/admin/auto-review/bulk-restore")]
async fn bulk_restore(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BulkRestoreReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_REVIEW,
    )
    .await?;
    let db = &state.repo.db;
    // 规则 → 拒因字典 sort（与 worker deny_reason_id_for 同表）
    let sort_for = match body.rule.as_str() {
        "arule_title_garbled" | "arule_title_format" => 91,
        "arule_descr_missing" => 92,
        "arule_ads_keywords" => 93,
        "arule_leak_marker" => 94,
        "arule_dupe_check" => 95,
        _ => 96,
    };
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT t.id FROM torrents t JOIN torrent_deny_reasons d ON d.id =  \
         t.deny_reason_id JOIN auto_review_log l ON l.torrent_id = t.id AND  \
         l.decided_by = 0 AND l.verdict = 1 AND EXISTS (SELECT 1 FROM  \
         jsonb_array_elements(l.hit_rules) e WHERE e->>'rule' = $1) WHERE  \
         t.approval_status = 2 AND d.sort = $2 AND t.mtime > now() -  \
         make_interval(days => $3) LIMIT 200",
    )
    .bind(&body.rule)
    .bind(sort_for)
    .bind(body.days)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut restored = 0i64;
    for tid in &rows {
        let n = sqlx::query(
            "UPDATE torrents SET approval_status = 0, deny_reason_id = NULL,  \
             deny_note = NULL WHERE id = $1 AND approval_status = 2",
        )
        .bind(tid)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n > 0 {
            let _ = sqlx::query(
                "INSERT INTO torrent_operation_logs (torrent_id, operator_id,  \
                 action, detail) VALUES ($1, $2, 'restore', $3::jsonb)",
            )
            .bind(tid)
            .bind(auth.id)
            .bind(
                serde_json::json!({"bulk_restore": true, "rule": body.rule})
                    .to_string(),
            )
            .execute(db)
            .await;
            // 台账释放：删掉机器行，让引擎按新规则重评（唯一索引约束）
            let _ = sqlx::query(
                "DELETE FROM auto_review_log WHERE torrent_id = $1 AND  \
                 decided_by = 0",
            )
            .bind(tid)
            .execute(db)
            .await;
            restored += n as i64;
        }
    }
    state
        .repo
        .audit(Some(auth.id), "review.auto_bulk_restore", None)
        .await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({
        "rule": body.rule, "restored": restored, "scanned": rows.len(),
    })))
}

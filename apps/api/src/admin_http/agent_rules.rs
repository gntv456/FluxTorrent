//! G-06 客户端黑白名单 admin API（NP AgentAllow/AgentDeny 口径）。
//! 0351 起规则带版本区间（version_min/max，点分数字或空=不限）与
//! priority（0..=1000，tracker 侧按 DESC 稳定排序先判）。
//! 校验拆至 agent_rule_check.rs、单测拆至 agent_rules_tests.rs（门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::agent_rule_check::{
    check_pattern, check_version_priority, AgentRuleReq,
};
use super::guard::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AgentRuleRow {
    id: i64,
    mode: String,
    pattern: String,
    note: Option<String>,
    version_min: String,
    version_max: String,
    priority: i32,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/agentrules")]
async fn agent_rules_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<AgentRuleRow> = sqlx::query_as(
        "SELECT r.id, r.mode, r.pattern, r.note, \
             COALESCE(r.version_min, ''), COALESCE(r.version_max, ''), \
             COALESCE(r.priority, 0), u.username AS created_by, r.created_at \
         FROM agent_rules r LEFT JOIN users u ON u.id = r.created_by \
         ORDER BY r.mode, r.id DESC LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/agentrules")]
async fn agent_rules_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if !["allow", "deny"].contains(&body.mode.as_str()) {
        return Err(DomainError::Validation("mode 取值 allow/deny".into()));
    }
    let p = body.pattern.trim();
    if p.is_empty() || p.len() > 100 {
        return Err(DomainError::Validation("pattern 长度 1-100".into()));
    }
    check_pattern(p)?;
    check_version_priority(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO agent_rules \
             (mode, pattern, note, version_min, version_max, priority, \
              created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(&body.mode)
    .bind(p)
    .bind(&body.note)
    .bind(body.version_min.trim())
    .bind(body.version_max.trim())
    .bind(body.priority)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "agentrule.add", Some(id))
        .await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct AgentRuleDel {
    id: i64,
}

// ---- U3 §12.3 反作弊规则包导入导出（JSON，站长圈共享） ----

/// 导出全部规则为可分享 JSON（allow/deny 分组）
#[get("/admin/agentrules/export")]
async fn agent_rules_export(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let rows: Vec<AgentRuleRow> = sqlx::query_as(
        "SELECT id, mode, pattern, note, \
             COALESCE(version_min, ''), COALESCE(version_max, ''), \
             COALESCE(priority, 0), NULL AS created_by, \
             now() AS created_at \
         FROM agent_rules ORDER BY mode, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let rules: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "mode": r.mode,
                "pattern": r.pattern,
                "note": r.note,
                "version_min": r.version_min,
                "version_max": r.version_max,
                "priority": r.priority,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "format": "fluxtorrent.agentrules.v1",
        "count": rules.len(),
        "rules": rules,
    })))
}

/// 导入规则包：合并去重（mode+pattern 相同跳过），返回新增数
#[derive(Deserialize)]
struct AgentRuleImportBody {
    rules: Vec<AgentRuleReq>,
}
#[post("/admin/agentrules/import")]
async fn agent_rules_import(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleImportBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.rules.len() > 500 {
        return Err(DomainError::Validation("单次导入上限 500 条".into()));
    }
    let mut added = 0i64;
    let mut invalid = 0i64;
    for r in &body.rules {
        if !["allow", "deny"].contains(&r.mode.as_str()) {
            continue; // 跳过非法条目，不整体失败
        }
        let p = r.pattern.trim();
        if p.is_empty() || p.len() > 100 {
            invalid += 1;
            continue; // 跳过非法条目，不整体失败
        }
        if check_pattern(p).is_err() || check_version_priority(r).is_err() {
            invalid += 1;
            continue;
        }
        let n = sqlx::query(
            "INSERT INTO agent_rules \
                 (mode, pattern, note, version_min, version_max, priority, \
                  created_by) \
             SELECT $1, $2, $3, $4, $5, $6, $7 \
             WHERE NOT EXISTS ( \
                   SELECT 1 FROM agent_rules \
                    WHERE mode = $1 AND pattern = $2)",
        )
        .bind(&r.mode)
        .bind(p)
        .bind(&r.note)
        .bind(r.version_min.trim())
        .bind(r.version_max.trim())
        .bind(r.priority)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        added += n as i64;
    }
    crate::http::bump_guard_ver(&state).await;
    state
        .repo
        .audit(Some(auth.id), "agentrule.import", Some(added))
        .await;
    Ok(ok(serde_json::json!({
        "added": added,
        "skipped": body.rules.len() as i64 - added,
        // 单独报出来：正则非法的规则存进去也不会生效，别让它假成功
        "invalid": invalid,
    })))
}

#[post("/admin/agentrules/delete")]
async fn agent_rules_del(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleDel>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置须 SETTINGS_MANAGE（与面板 min_class=99 一致）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query("DELETE FROM agent_rules WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state
        .repo
        .audit(Some(auth.id), "agentrule.del", Some(body.id))
        .await;
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}

#[cfg(test)]
#[path = "agent_rules_tests.rs"]
mod tests;

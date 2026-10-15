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

/// 备注长度上限（与 `audit_detail` 的 200 字口径一致）。**拒绝而不是静默
/// 截断**——截掉的正是纠纷仲裁时要看的半句（同 P-156/157）。
const NOTE_MAX: usize = 200;

/// 单条规则的校验尺，`add` 与 `import` 共用（批五十一 P-195：原先 import
/// 把同一把尺降级成「静默跳过」，导入 20 条只进 3 条也回 200，站长看不出
/// 哪条没进去）。`Ok(归一化 pattern)` / `Err(原话)`。
fn validate_rule(r: &AgentRuleReq) -> Result<String, String> {
    if !["allow", "deny"].contains(&r.mode.as_str()) {
        return Err("mode 取值 allow/deny".into());
    }
    let p = r.pattern.trim();
    // 长度按**字符**判：原先用 `len()`（字节），一条三十几字的中文 UA 规则
    // 会被「pattern 长度 1-100」自拒（同 P-137/P-23 那一族）
    if p.is_empty() || p.chars().count() > 100 {
        return Err("pattern 长度 1-100 字".into());
    }
    let note = r.note.clone().unwrap_or_default();
    if p.contains('\u{0}') || note.contains('\u{0}') {
        return Err("pattern / 备注不允许包含空字节".into());
    }
    if note.chars().count() > NOTE_MAX {
        return Err(format!(
            "备注不得超过 {NOTE_MAX} 字（收到 {} 字）",
            note.chars().count()
        ));
    }
    check_pattern(p).map_err(|e| e.to_string())?;
    check_version_priority(r).map_err(|e| e.to_string())?;
    Ok(p.to_string())
}

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
    let auth = staff(&req, &state).await?;
    // P-338（批一百五十四）：本读口此前只过粗闸 staff()，而同文件的写口与
    // 兄弟端点都要「SETTINGS_MANAGE」（class 99 起）。后果：class 91「发布员」能读
    // 智能体规则读口全表，写却被 403——**读比写松**，属权限倒挂。
    // 取证：user_can(uid, "settings.manage") 对 class 91 返回 false，
    // 而 GET 这条仍回 200（实测 91 档拿到 email-domains 200 行、
    // roles-dict 5 行、agentrules 12 条）。
    // 修法：读口与写口对齐同一把权限键；语义上「读配置」与「改配置」同档。
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    // sqlx 的 `FromRow` 按**列名**取值，而 `COALESCE(...)` 的默认输出列名是
    // `coalesce`（不是 `version_min`）——0351 加这三列时没起别名，于是这条
    // SELECT 对任何人都 500「no column found for name: version_min」，
    // 后台「客户端黑白名单」整页打不开（批五十一 P-193，编译器看不见串内形状）。
    let rows: Vec<AgentRuleRow> = sqlx::query_as(
        "SELECT r.id, r.mode, r.pattern, r.note, \
             COALESCE(r.version_min, '') AS version_min, \
             COALESCE(r.version_max, '') AS version_max, \
             COALESCE(r.priority, 0) AS priority, \
             u.username AS created_by, r.created_at \
         FROM agent_rules r LEFT JOIN users u ON u.id = r.created_by \
         ORDER BY r.mode, r.id DESC LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(crate::openapi_http::no_store(ok(rows)))
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
    let p = validate_rule(&body).map_err(DomainError::Validation)?;
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
             COALESCE(version_min, '') AS version_min, \
             COALESCE(version_max, '') AS version_max, \
             COALESCE(priority, 0) AS priority, NULL AS created_by, \
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
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "format": "fluxtorrent.agentrules.v1",
        "count": rules.len(),
        "rules": rules,
    }))))
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
    // 整批一个事务（批五十一 P-194）：原先逐条 autocommit，中途一条撞库错
    // （实测：备注里带 \\u0000 → Postgres `invalid byte sequence`）就回 500，
    // 而**前面已插入的规则留在库里** —— 站长看到的是一次失败的导入留下了
    // 半包规则，重放还会再叠一层。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut added = 0i64;
    let mut dups = 0i64;
    let mut rejected: Vec<serde_json::Value> = Vec::new();
    for (i, r) in body.rules.iter().enumerate() {
        let p = match validate_rule(r) {
            Ok(p) => p,
            Err(w) => {
                rejected.push(serde_json::json!({ "index": i, "reason": w }));
                continue;
            }
        };
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
        .bind(&p)
        .bind(&r.note)
        .bind(r.version_min.trim())
        .bind(r.version_max.trim())
        .bind(r.priority)
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n == 0 {
            dups += 1;
        }
        added += n as i64;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    crate::http::bump_guard_ver(&state).await;
    state
        .repo
        .audit(Some(auth.id), "agentrule.import", Some(added))
        .await;
    Ok(ok(serde_json::json!({
        "added": added,
        "skipped": body.rules.len() as i64 - added,
        // 单独报出来：非法的规则存进去也不会生效，别让它假成功
        "invalid": rejected.len() as i64,
        // 批五十一 P-195：原先只给计数、连 mode 写错都不计，这里逐条点名
        "rejected": rejected,
        "duplicates": dups,
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

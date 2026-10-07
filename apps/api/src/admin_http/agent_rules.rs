use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ G-06 客户端黑白名单（NP AgentAllow/AgentDeny 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AgentRuleRow {
    id: i64,
    mode: String,
    pattern: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// pattern 会被 tracker 编译成正则（`guard_refresh.rs` 里 `Regex::new`），
/// 编译失败就在加载阶段被跳过——站点侧看起来「已配置」，实际恒不生效。
/// 所以在写入口就判掉：允许合法正则，拒绝非法语法与空模式。
pub(crate) fn check_pattern(p: &str) -> DomainResult<()> {
    // 详情（编译器报错）走日志，不进用户可见串：format! 拼的句子无法按原句查表
    regex::Regex::new(p)
        .map_err(|e| {
            tracing::warn!(%e, %p, "agent_rules pattern 不是合法正则");
            DomainError::Validation("pattern 需是合法正则表达式".into())
        })
        .map(|_| ())
}

#[get("/admin/agentrules")]
async fn agent_rules_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<AgentRuleRow> = sqlx::query_as(
        "SELECT r.id, r.mode, r.pattern, r.note, u.username AS created_by, r.created_at \
         FROM agent_rules r LEFT JOIN users u ON u.id = r.created_by \
         ORDER BY r.mode, r.id DESC LIMIT 200",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AgentRuleReq {
    mode: String,
    pattern: String,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/agentrules")]
async fn agent_rules_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
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
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO agent_rules (mode, pattern, note, created_by) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&body.mode)
    .bind(p)
    .bind(&body.note)
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
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT mode, pattern, note FROM agent_rules ORDER BY mode, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let rules: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(mode, pattern, note)| {
            serde_json::json!({"mode": mode, "pattern": pattern, "note": note})
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
        if check_pattern(p).is_err() {
            invalid += 1;
            continue;
        }
        let n = sqlx::query(
            "INSERT INTO agent_rules (mode, pattern, note, created_by) \
             SELECT $1, $2, $3, $4 \
             WHERE NOT EXISTS (SELECT 1 FROM agent_rules WHERE mode = $1 AND pattern = $2)",
        )
        .bind(&r.mode)
        .bind(p)
        .bind(&r.note)
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
    Ok(ok(
        serde_json::json!({
            "added": added,
            "skipped": body.rules.len() as i64 - added,
            // 单独报出来：正则非法的规则存进去也不会生效，别让它假成功
            "invalid": invalid,
        }),
    ))
}

#[post("/admin/agentrules/delete")]
async fn agent_rules_del(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AgentRuleDel>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：客户端名单/拒绝原因是站点级配置，须 SETTINGS_MANAGE（与面板 min_class=99 一致）
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
mod tests {
    use super::check_pattern;

    #[test]
    fn literal_substring_still_compiles() {
        // 0036 注释举的例子：子串写法本身就是合法正则
        assert!(check_pattern("Transmission/3").is_ok());
        assert!(check_pattern("^qBittorrent/4\\.6").is_ok());
    }

    #[test]
    fn invalid_regex_is_rejected_at_the_write_door() {
        // 站长按「前缀匹配」直觉填的括号写法——旧版能存进去，但永远不生效
        for bad in ["uTorrent/3.5.5 (build)", "(", "a{2,1}", "[z-a]", "*x"] {
            assert!(
                check_pattern(bad).is_err(),
                "{bad} 应在写入口就被拒，而不是加载时静默跳过"
            );
        }
    }
}

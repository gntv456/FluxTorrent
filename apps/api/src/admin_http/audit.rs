use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 审计日志 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AuditRow {
    id: i64,
    actor_id: Option<i64>,
    action: String,
    /// 审计现场（0285）：目标 id / 理由 / 附加上下文。旧版读模型不选这列，
    /// 面板只能看到「user.set_class」而不知道对谁做、为什么做。
    detail: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
struct AuditQ {
    /// action 模糊匹配（旧口径保留：面板搜索框直接用这个）
    #[serde(default = "empty_q")]
    q: String,
    /// 按**对象**回查：`ref->>'id'` 等于该用户 id。
    /// 「这个号被谁动过、动过什么」是发放/调账纠纷里第一问，
    /// 旧版只能靠 action 关键词搜，200 行一封顶就翻不到了。
    #[serde(default)]
    uid: Option<i64>,
    /// 按操作者回查（交接：某个版主做过哪些动作）
    #[serde(default)]
    actor: Option<i64>,
    /// 行数上限：默认 200，最多 1000（批量发放后翻旧记录要用）
    #[serde(default = "default_audit_limit")]
    limit: i64,
}

fn empty_q() -> String {
    String::new()
}

fn default_audit_limit() -> i64 {
    200
}

#[get("/admin/audit")]
async fn audit_query(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AuditQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AUDIT_VIEW)
        .await?;
    if !(1..=1000).contains(&q.limit) {
        return Err(DomainError::Validation("limit 取值 1-1000".into()));
    }
    // 空串 = 不过滤。旧版把空串喂给 like_pattern 得到 '%%'，语义上等价，
    // 但显式传 NULL 才能和 uid/actor 三个条件写成同一组可选谓词。
    let pattern = if q.q.trim().is_empty() {
        None
    } else {
        Some(crate::http::like_pattern(&q.q))
    };
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT id, actor_id, action, \
         ref::text AS detail, created_at FROM audit_log \
         WHERE ($1::text IS NULL OR action ILIKE $1) \
           AND ($2::text IS NULL OR ref->>'id' = $2) \
           AND ($3::bigint IS NULL OR actor_id = $3) \
         ORDER BY id DESC LIMIT $4",
    )
    .bind(pattern.as_deref())
    .bind(q.uid.map(|v| v.to_string()))
    .bind(q.actor)
    .bind(q.limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 审计哈希链校验（0266 实装）：逐行重算 self_hash 并验证与后继行 prev_hash 的串接。
/// 返回总行数与第一条断链行 id（null = 全链完整）。给站长「审计可信」的一键取证。
#[derive(sqlx::FromRow, serde::Serialize)]
struct ChainVerify {
    total: i64,
    first_broken_id: Option<i64>,
}

#[get("/admin/audit/chain-verify")]
async fn audit_chain_verify(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AUDIT_VIEW)
        .await?;
    // 摘要公式与触发器 audit_log_chain()（0266）完全一致；
    // prev_bad = 本行 prev_hash ≠ 前一行（按 id 序）self_hash（首行对照全零创世）。
    let row: ChainVerify = sqlx::query_as(
        r#"
        WITH ordered AS (
            SELECT id, prev_hash, self_hash,
                   digest(
                       COALESCE(prev_hash, decode(repeat('00', 32), 'hex')) ||
                       id::text::bytea ||
                       COALESCE(actor_id::text, 'null')::bytea ||
                       action::bytea ||
                       COALESCE(ref::text, 'null')::bytea ||
                       to_char(created_at AT TIME ZONE 'UTC',
                               'YYYY-MM-DD HH24:MI:SS.US')::bytea,
                       'sha256') AS expect_hash,
                   lag(self_hash) OVER (ORDER BY id) AS prev_self,
                   decode(repeat('00', 32), 'hex') AS genesis
            FROM audit_log
        )
        SELECT (SELECT count(*) FROM audit_log) AS total,
               (SELECT min(id) FROM ordered
                WHERE self_hash IS DISTINCT FROM expect_hash
                   OR prev_hash IS DISTINCT FROM COALESCE(prev_self, genesis)
               ) AS first_broken_id
        "#,
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(row))
}

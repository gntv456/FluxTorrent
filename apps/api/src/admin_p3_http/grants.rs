//! 发放台账（0291）：批量与单发同写 `grant_batches`，读取走 `GET /admin/grants`。
//!
//! 两条写侧口径，都是实测逼出来的：
//! 1. **占位先于执行**。一批发放先在 INSERT 上落库，幂等键撞唯一约束即拒
//!    （不是 SELECT-then-INSERT——并发双击两道都能查到「没用过」，实测
//!    同参数连发两次魔力 +200）。进程崩在中间也留下一行 `pending`。
//! 2. **执行结果必须回写**。分批发放中途失败时，前面几批是**真发出去了**的；
//!    只回一个 500 就等于让站长在「以为没发」和「实际发了一半」之间猜。
//!    回写 `partial` + error，界面上看得见这批发到哪一步。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

/// 一批发放的计划现场。
pub(crate) struct BatchSpec<'a> {
    pub batch_id: &'a str,
    pub idem: Option<&'a str>,
    pub actor_id: i64,
    pub kind: &'a str,
    pub amount: i64,
    pub item_id: Option<i64>,
    pub medal_id: Option<i64>,
    pub days: Option<i32>,
    pub selector: serde_json::Value,
    pub targets: &'a [i64],
    pub subject: Option<&'a str>,
}

/// 占位落库。幂等键重复 → 400 并说明是哪把键；其余 DB 错误照常上抛。
pub(crate) async fn batch_open(
    db: &PgPool,
    spec: &BatchSpec<'_>,
) -> DomainResult<()> {
    let r = sqlx::query(
        "INSERT INTO grant_batches (batch_id, idempotency_key, actor_id, \
         kind, amount, item_id, medal_id, days, selector, target_ids, \
         subject) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(spec.batch_id)
    .bind(spec.idem)
    .bind(spec.actor_id)
    .bind(spec.kind)
    .bind(spec.amount)
    .bind(spec.item_id)
    .bind(spec.medal_id)
    .bind(spec.days)
    .bind(&spec.selector)
    .bind(spec.targets)
    .bind(spec.subject)
    .execute(db)
    .await;
    if let Err(e) = r {
        // batch_id 是 uuid4，实务上不会撞；撞了也只可能是幂等键那条约束
        let dup = matches!(&e, sqlx::Error::Database(d)
            if d.code().as_deref() == Some("23505"));
        if dup && spec.idem.is_some() {
            return Err(DomainError::Validation(
                "该 idempotency_key 已用于一批发放，已拒绝重复提交".into(),
            ));
        }
        return Err(DomainError::Internal(e.into()));
    }
    Ok(())
}

/// 回写执行结果。返回 Err 时调用方**不能**把发放本身判成失败——
/// 钱已经动了，只能在响应里带一条 ledger 告警，否则站长会重发一遍。
pub(crate) async fn batch_finish(
    db: &PgPool,
    batch_id: &str,
    affected: i64,
    status: &str,
    error: Option<&str>,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE grant_batches SET affected = $2, status = $3, error = $4 \
         WHERE batch_id = $1",
    )
    .bind(batch_id)
    .bind(affected)
    .bind(status)
    .bind(error)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct GrantRow {
    id: i64,
    batch_id: String,
    actor_id: i64,
    actor_name: String,
    kind: String,
    amount: i64,
    item_id: Option<i64>,
    item_name: Option<String>,
    medal_id: Option<i64>,
    medal_name: Option<String>,
    days: Option<i32>,
    targets: i64,
    /// 表里是 `integer`(INT4)，这里就必须是 i32——sqlx 解码按 SQL 实际类型
    /// 判，写成 i64 会当场 mismatched types 让台账接口 500（实测 F1）。
    affected: i32,
    status: String,
    error: Option<String>,
    subject: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct GrantQ {
    /// 按批次号精确回放（漏发核对入口）
    #[serde(default)]
    batch: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    /// 按操作者过滤（交接时「这个号发过什么」）
    #[serde(default)]
    actor: Option<i64>,
    /// pending / done / partial / failed
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

/// 看台账的门槛：能发东西的人必须能查账，否则「发完查不到」是逼人去读库。
async fn can_view(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
) -> bool {
    for p in [
        crate::authz::perm::PROP_MANAGE,
        crate::authz::perm::MEDAL_MANAGE,
        crate::authz::perm::USER_ADJUST,
        crate::authz::perm::USER_AMOUNTBONUS,
        crate::authz::perm::AUDIT_VIEW,
    ] {
        if crate::authz::can(state, auth, p).await {
            return true;
        }
    }
    false
}

#[get("/admin/grants")]
async fn admin_grants(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<GrantQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if !can_view(&state, &auth).await {
        return Err(DomainError::Forbidden);
    }
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let (off, lim) = crate::dto::page_window(q.page, q.per_page);
    let rows: Vec<GrantRow> = sqlx::query_as(
        r#"SELECT g.id, g.batch_id, g.actor_id,
                  coalesce(u.username, '(已注销)') AS actor_name,
                  g.kind, g.amount, g.item_id, i.name AS item_name,
                  g.medal_id, m.name AS medal_name, g.days,
                  coalesce(array_length(g.target_ids, 1), 0)::bigint
                      AS targets,
                  g.affected, g.status, g.error, g.subject, g.created_at
           FROM grant_batches g
           LEFT JOIN users u ON u.id = g.actor_id
           LEFT JOIN shop_items i ON i.id = g.item_id
           LEFT JOIN medals m ON m.id = g.medal_id
           WHERE ($1::text IS NULL OR g.batch_id = $1)
             AND ($2::text IS NULL OR g.kind = $2)
             AND ($3::bigint IS NULL OR g.actor_id = $3)
             AND ($4::text IS NULL OR g.status = $4)
           ORDER BY g.id DESC LIMIT $5 OFFSET $6"#,
    )
    .bind(q.batch.as_deref())
    .bind(q.kind.as_deref())
    .bind(q.actor)
    .bind(q.status.as_deref())
    .bind(lim)
    .bind(off)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM grant_batches g WHERE ($1::text IS NULL OR \
         g.batch_id = $1) AND ($2::text IS NULL OR g.kind = $2) \
         AND ($3::bigint IS NULL OR g.actor_id = $3) \
         AND ($4::text IS NULL OR g.status = $4)",
    )
    .bind(q.batch.as_deref())
    .bind(q.kind.as_deref())
    .bind(q.actor)
    .bind(q.status.as_deref())
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let mut out = serde_json::json!({
        "rows": rows, "total": total,
        "page": q.page.max(1), "per_page": q.per_page,
    });
    // 按批次回放时补上完整受众（含用户名）——漏发核对看的就是这一列
    if let Some(b) = q.batch.as_deref() {
        let first = out
            .get_mut("rows")
            .and_then(|v| v.as_array_mut())
            .and_then(|a| a.first_mut());
        if let Some(first) = first {
            let people: Vec<(i64, String)> = sqlx::query_as(
                "SELECT t.uid, coalesce(u.username, '(已注销)') \
                 FROM grant_batches g \
                 CROSS JOIN LATERAL unnest(g.target_ids) AS t(uid) \
                 LEFT JOIN users u ON u.id = t.uid \
                 WHERE g.batch_id = $1 ORDER BY t.uid",
            )
            .bind(b)
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
            let ids: Vec<i64> = people.iter().map(|p| p.0).collect();
            first["target_ids"] = serde_json::json!(ids);
            first["target_users"] = serde_json::json!(people
                .iter()
                .map(|p| serde_json::json!({
                    "id": p.0, "username": p.1,
                }))
                .collect::<Vec<_>>());
        }
    }
    Ok(ok(out))
}

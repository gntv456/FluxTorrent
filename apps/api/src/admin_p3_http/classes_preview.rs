//! 等级门槛改动的影响预览（0358 同批：无迁移，纯只读端点）。
//! 从 classes_admin.rs 拆出（300 行门禁）；权限/档位校验同 PUT 口径。
//!
//! 判定 SQL 照搬 worker class_auto_adjust 的 CTE（agg → stats → target），
//! 唯一差别：目标档的规则行来自**请求体假设值**（VALUES 单行）而非库中
//! class_rules，其余档位仍用库中现值——站长预览的就是「未保存的草稿」。
//!
//! 口径（简化，与 worker 实际执行一致）：
//! - promote：按「全档位规则 + 假设行替换目标档」算出的新 target >
//!   用户当前档。用户可能中间档也满足（worker 逐级取 max，这里同样
//!   取 max，故升到几档的中间态不会重复计数）。
//! - demote：套 demotable 语义（假设行的 demotable + 其余档库中值）；
//!   一条规则都不满足 → 保底 1（worker 同口径，不降为 0）。
//! - staff（class_id >= 90）天然排除（stats CTE 的 u.class_id < 90）。
//!
//! 性能：snatches 全表 GROUP BY 是重型操作（worker 侧同款），同参数
//! 60s 内直接回 Redis 缓存（throttle.rs 的 redis 连接；fail-open——
//! 缓存层故障不应把只读预览打成 500，宁可重算）。

use std::sync::Arc;

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::classes_admin::STAFF_CLASS_FLOOR;

#[derive(Deserialize)]
pub(crate) struct ClassImpactBody {
    /// 四维阈值假设值 + demotable：与 ClassPutBody 同六字段的子集
    /// （name/promo_sparks 不参与升降判定，预览不需要）
    min_uploaded: i64,
    min_download_count: i32,
    min_seed_hours: i32,
    min_account_age_days: i32,
    #[serde(default)]
    demotable: bool,
}

/// 结果缓存键：全参数指纹（同参数 60s 内直接回缓存）
fn cache_key(id: i32, b: &ClassImpactBody) -> String {
    format!(
        "clsimpact:{}:{}:{}:{}:{}:{}",
        id,
        b.min_uploaded,
        b.min_download_count,
        b.min_seed_hours,
        b.min_account_age_days,
        b.demotable
    )
}

/// 读写 60s 结果缓存（Redis；fail-open：故障时视为未命中/不写）。
async fn cache_get(
    state: &Arc<AppState>,
    key: &str,
) -> Option<serde_json::Value> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let raw: Option<String> = c.get(key).await.ok()?;
    let s = raw.as_deref()?;
    serde_json::from_str(s).ok()
}

async fn cache_put(state: &Arc<AppState>, key: &str, v: &serde_json::Value) {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let touched: redis::RedisResult<()> = c
        .set_ex(key, serde_json::to_string(v).unwrap_or_default(), 60)
        .await;
    if let Err(e) = touched {
        tracing::warn!(%e, key, "等级预览缓存写入失败（只影响节流）");
    }
}

/// POST /admin/classes/{id}/impact-preview：按假设阈值预览升降集合。
/// 只读（不改 users）；返回升降计数 + 样例名单 + 该档现有人数。
#[post("/admin/classes/{id}/impact-preview")]
pub async fn admin_class_impact_preview(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<ClassImpactBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    // 与 PUT 同口径：1..=12 用户档可预览；staff 档/无规则档拒绝
    if id >= STAFF_CLASS_FLOOR || id < 1 {
        return Err(DomainError::Validation(
            "仅开放 1-12 用户档预览（staff 档位不参与升降判定）".into(),
        ));
    }
    let has_rule: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM class_rules WHERE class_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !has_rule {
        return Err(DomainError::Validation(
            "该档位没有升降级规则（贵宾/职务档不开放预览）".into(),
        ));
    }
    if body.min_uploaded < 0
        || body.min_download_count < 0
        || body.min_seed_hours < 0
        || body.min_account_age_days < 0
    {
        return Err(DomainError::Validation("阈值不能为负".into()));
    }
    // 防滥用节流（同端点 30 次/分，对齐管理端重操作口径）；
    // fail-close 与 throttle.rs 全局纪律一致
    crate::http::throttle_limited(
        &state,
        format!("clsimpact:{}:{}", auth.id, id),
        30,
    )
    .await?;
    let key = cache_key(id, &body);
    if let Some(cached) = cache_get(&state, &key).await {
        return Ok(ok(cached));
    }
    // 该档当前人数（改前心里有数；GET 列表同款子查询）
    let current_users: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE class_id = $1")
            .bind(id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 判定主体：worker class_auto_adjust 的 CTE 只读版。
    // rules = 库中其余档 ∪ 假设行（替换目标档）：target 取 max 满足档
    // （一条不满足保底 1，worker 同口径）；demote 半边还要求用户**当前档**
    // demotable（当前档 = 目标档时用假设值，其余档用库中值）——关掉
    // demotable 的档成员不会被降，与 worker demote CTE 的过滤一致。
    let rows: Vec<(i64, String)> = sqlx::query_as(
        r#"
        WITH agg AS (
            SELECT user_id,
                   count(*) FILTER (WHERE completed_at IS NOT NULL) AS dl,
                   COALESCE(sum(seeded_seconds), 0) / 3600 AS sh
            FROM snatches GROUP BY user_id
        ),
        stats AS (
            SELECT u.id, u.username, u.class_id, u.uploaded,
                   COALESCE(a.dl, 0)::bigint AS dl,
                   COALESCE(a.sh, 0)::bigint AS sh,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u LEFT JOIN agg a ON a.user_id = u.id
            WHERE u.class_id < 90
        ),
        rules AS (
            -- 其余档用库中规则；目标档替换为请求体假设行
            SELECT class_id, min_uploaded, min_download_count,
                   min_seed_hours, min_account_age_days, demotable
            FROM class_rules WHERE class_id <> $1
            UNION ALL
            SELECT $1, $2, $3, $4, $5, $6
        ),
        target AS (
            -- max 满足档；LEFT JOIN + 保底 1（worker demote 半边同口径）
            SELECT s.id, s.username, s.class_id,
                   COALESCE(max(r.class_id), 1) AS new_class
            FROM stats s LEFT JOIN rules r ON
                s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count
                AND s.sh >= r.min_seed_hours
                AND s.age >= r.min_account_age_days
            GROUP BY s.id, s.username, s.class_id
        )
        SELECT t.new_class - t.class_id, t.username
        FROM target t JOIN rules cr ON cr.class_id = t.class_id
        WHERE t.new_class > t.class_id
           OR (t.new_class < t.class_id AND cr.demotable)
        ORDER BY t.new_class - t.class_id DESC, t.id
        "#,
    )
    .bind(id)
    .bind(body.min_uploaded)
    .bind(body.min_download_count)
    .bind(body.min_seed_hours)
    .bind(body.min_account_age_days)
    .bind(body.demotable)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut promote_n: i64 = 0;
    let mut demote_n: i64 = 0;
    let mut sample_promote: Vec<String> = Vec::new();
    let mut sample_demote: Vec<String> = Vec::new();
    for (diff, username) in rows {
        if diff > 0 {
            promote_n += 1;
            if sample_promote.len() < 10 {
                sample_promote.push(username);
            }
        } else {
            demote_n += 1;
            if sample_demote.len() < 10 {
                sample_demote.push(username);
            }
        }
    }
    let out = json!({
        "promote": promote_n,
        "demote": demote_n,
        "sample_promote": sample_promote,
        "sample_demote": sample_demote,
        "current_users": current_users,
    });
    cache_put(&state, &key, &out).await;
    Ok(ok(out))
}

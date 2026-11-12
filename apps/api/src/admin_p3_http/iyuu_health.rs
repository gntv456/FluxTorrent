//! IYUU 辅种缓存健康（P2-d 站长观测）。
//!
//! 辅种面板的可用性完全取决于 `iyuu_crossseed` 表的健康度，但站长此前
//! 只能看 tracing 日志。本端点一条聚合 SQL 回四个数：覆盖多少、命中
//! 多少、失败多少、最老一行多旧——配额消耗是否合理、worker 是否在跑、
//! 死链复核是否跟得上，一眼可判。
//!
//! 只读、无副作用；鉴权与任务调度面板（ops_jobs）同口径。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
struct IyuuHealthRow {
    /// iyuu_crossseed 总行数（已查过至少一次的种子数）
    rows_total: i64,
    /// 有命中的行数（面板正在展示的数据）
    rows_hit: i64,
    /// ok=false 的行数（最近一次查询失败的残留）
    rows_failed: i64,
    /// 最老一行的 fetched_at（NULL = 表空）
    oldest_fetched: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/iyuu_health",
    tag = "辅种(crossseed)",
    description = "IYUU 辅种缓存健康快照：覆盖/命中/失败行数与最老刷新时间。\
            rows_failed 持续增长 = 凭据或配额出问题；oldest_fetched 很旧\
            = worker 没在跑或 batch 太小。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "无权限（code 1003）"),
    )
)]
#[get("/admin/iyuu_health")]
pub(crate) async fn admin_iyuu_health(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let row: IyuuHealthRow = sqlx::query_as(
        "SELECT COUNT(*)::bigint AS rows_total, \
                COUNT(*) FILTER (WHERE jsonb_array_length(hits) > 0)::bigint \
                    AS rows_hit, \
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS rows_failed, \
                MIN(fetched_at) AS oldest_fetched \
         FROM iyuu_crossseed",
    )
    .fetch_one(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 顺手回配置态（enabled/凭据齐全与否），站长排查少一次跳转
    let conf = crate::iyuu_bridge::conf(&state).await;
    Ok(ok(serde_json::json!({
        "rows_total": row.rows_total,
        "rows_hit": row.rows_hit,
        "rows_failed": row.rows_failed,
        "oldest_fetched_at": row.oldest_fetched,
        // 全站已过审种数：与 rows_total 一比就是覆盖率
        "approved_torrents": approved_count(&state).await,
        "enabled": conf.enabled,
        "configured": conf.ready(),
    })))
}

/// 已过审种总数（覆盖率分母）。
async fn approved_count(state: &AppState) -> i64 {
    let n: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM torrents WHERE approval_status = 1",
    )
    .fetch_one(&state.repo.read_db)
    .await
    .unwrap_or(None);
    n.unwrap_or(0)
}

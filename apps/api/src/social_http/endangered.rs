//! 濒危预警雷达（0102，从 social_http.rs 按域拆出）。
//!
//! 定位：预警雷达（前置保种）——对象是「濒危但有救」的资源（seeders <= 阈值），
//! 只读展示 + 引导去保种；与 ops_http 的 resurrections（死种复活，后置抢救）
//! 互补，不重复实现。濒危是动态状态，不落清单表，实时查 torrents。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 模块开关：module_social = 'no' 时关闭。
/// 缺省（未配置）视为关闭 —— 通用程序不应默认暴露社区玩法。
pub async fn social_disabled(db: &sqlx::PgPool) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_social'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|v| v == "no")
    .unwrap_or(true)
}

/// 读取整数型站点设置（缺省取 dft）
pub async fn setting_i64(db: &sqlx::PgPool, name: &str, dft: i64) -> i64 {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(dft)
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<i64>,
    size: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct EndangeredItem {
    torrent_id: i64,
    info_hash: String,
    name: String,
    seeders: i32,
    leechers: i32,
    times_completed: i32,
    size: i64,
    category_id: i32,
    age_days: f64,
    /// 是否已有进行中的复活任务（resurrections.status = 'open'）
    rescue_open: bool,
}

#[derive(Serialize)]
struct ListResp {
    enabled: bool,
    /// 濒危阈值（做种数 <= 该值列入雷达）
    endangered_seeders: i64,
    /// 健康阈值（对齐保种区移出口径），用于前端展示「距健康还差几个做种者」
    health_seeders: i64,
    page: i64,
    size: i64,
    total: i64,
    list: Vec<EndangeredItem>,
}

/// 濒危预警雷达（只读）。
///
/// 不建清单表：濒危是动态状态（今天濒危、明天可能已被保种），落表反而要处理时效性，
/// 直接实时查 torrents 即可。排序「越濒危越靠前」：做种数升序 → 完成数降序 → 体积降序。
#[get("/social/endangered/list")]
async fn endangered_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;

    let endangered_seeders =
        setting_i64(&state.repo.db, "social_endangered_seeders", 1).await;
    let health_seeders =
        setting_i64(&state.repo.db, "social_health_seeders", 7).await;

    // 模块关闭：返回 enabled=false + 空列表（前端隐藏入口，不报错、不引导）
    if social_disabled(&state.repo.db).await {
        return Ok(ok(ListResp {
            enabled: false,
            endangered_seeders,
            health_seeders,
            page: 1,
            size: 20,
            total: 0,
            list: Vec::new(),
        }));
    }

    let page = q.page.unwrap_or(1).max(1);
    let size = q.size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * size;

    // count 与列表必须同套谓词，否则总数与实际行数不符
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents t \
         WHERE t.approval_status = 1 AND t.seeders <= $1 AND t.seeders > 0 AND t.times_completed >= 3",
    )
    .bind(endangered_seeders)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let list: Vec<EndangeredItem> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.info_hash, t.name, t.seeders, t.leechers, \
                t.times_completed, t.size, t.category_id, \
                (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::float8 AS age_days, \
                (r.id IS NOT NULL) AS rescue_open \
         FROM torrents t \
         LEFT JOIN resurrections r ON r.torrent_id = t.id AND r.status = 'open' \
         WHERE t.approval_status = 1 AND t.seeders <= $1 AND t.seeders > 0 AND t.times_completed >= 3 \
         ORDER BY t.seeders ASC, t.times_completed DESC, t.size DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(endangered_seeders)
    .bind(size)
    .bind(offset)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(ListResp {
        enabled: true,
        endangered_seeders,
        health_seeders,
        page,
        size,
        total,
        list,
    }))
}

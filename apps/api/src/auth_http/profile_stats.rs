//! 公开主页统计卡（0503）：GET /users/{id}/stats。
//!
//! 10-26 体检 B 画像遗留「他人主页无任何可视化」——只有资料卡与
//! 列表。本端点聚合三块纯现有列（零新表零新列）：
//!   · 做种构成：在种体积/颗数/时长 vs 已完成下载数（分享率画像）
//!   · 类别热点：本人种子按分类的颗数/体积 TOP8（兴趣画像）
//!   · 做种时长分布：snatches.seeded_seconds 分桶（贡献深度画像）
//! 权限与 user_public_profile 同口径（require_auth + 同款限流）；
//! 隐私不新开口——列出的都是 torrentlist 页本就可见的聚合。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/users/{id}/stats")]
pub async fn user_public_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::http::throttle_limited(&state, format!("profile:{}", auth.id), 60)
        .await?;
    let uid = path.into_inner();
    // 主角存在性（与 profile 同款 404 语义）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status < 2)",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(uid));
    }
    // 1) 做种构成（在种聚合 + 完成下载计数）
    let (seed_bytes, seed_count, seed_hours, dl_done): (
        i64, i64, i64, i64,
    ) = sqlx::query_as(
        "SELECT COALESCE(sum(t.size) FILTER (WHERE s.seeding), 0)::bigint, \
             count(*) FILTER (WHERE s.seeding)::bigint, \
             COALESCE(sum(s.seeded_seconds), 0)::bigint / 3600, \
             count(*) FILTER (WHERE s.completed_at IS NOT NULL)::bigint \
         FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 AND t.approval_status = 1",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 2) 类别热点 TOP8（含名称；未配分类名回退 id）
    let cats: Vec<(i32, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT t.category_id, max(c.name), count(*)::bigint, \
             COALESCE(sum(t.size), 0)::bigint \
         FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE s.user_id = $1 AND s.seeding AND t.approval_status = 1 \
         GROUP BY t.category_id \
         ORDER BY sum(t.size) DESC NULLS LAST LIMIT 8",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 3) 做种时长分布（分桶：<1h / 1-24h / 1-7d / 7-30d / >30d）
    let (b1, b2, b3, b4, b5): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           count(*) FILTER (WHERE s.seeded_seconds < 3600)::bigint, \
           count(*) FILTER (WHERE s.seeded_seconds >= 3600 \
                        AND s.seeded_seconds < 86400)::bigint, \
           count(*) FILTER (WHERE s.seeded_seconds >= 86400 \
                        AND s.seeded_seconds < 604800)::bigint, \
           count(*) FILTER (WHERE s.seeded_seconds >= 604800 \
                        AND s.seeded_seconds < 2592000)::bigint, \
           count(*) FILTER (WHERE s.seeded_seconds >= 2592000)::bigint \
         FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1 AND s.seeding AND t.approval_status = 1",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "seeding": {
            "bytes": seed_bytes, "count": seed_count,
            "hours": seed_hours, "downloaded_done": dl_done,
        },
        "categories": cats.iter().map(|(id, name, n, sz)| {
            let label = name.clone()
                .unwrap_or_else(|| format!("#{id}"));
            serde_json::json!({
                "id": id,
                "name": label,
                "count": n, "bytes": sz,
            })
        }).collect::<Vec<_>>(),
        "seed_buckets": [
            {"k": "lt1h", "n": b1}, {"k": "h1_24", "n": b2},
            {"k": "d1_7", "n": b3}, {"k": "d7_30", "n": b4},
            {"k": "gt30d", "n": b5},
        ],
    })))
}

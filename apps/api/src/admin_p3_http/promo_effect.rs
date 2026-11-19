//! 促销效果归因（0503 运营台账 #1）：GET /admin/promo-effect。
//! 每条近期促销统计窗口内「完成下载数 + 独立下载者」（scope 匹配口径
//! 与前端展示同源：global=全站、category=该分类、official/non_official
//! 按官种标记）——站长此前只能跨表 SQL 拼归因。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/admin/promo-effect")]
pub async fn promo_effect(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_VIEW)
        .await?;
    // 近 10 条促销的窗口内完成下载归因（scope 匹配在 SQL 侧逐条 CASE）
    // 近 10 条促销逐条归因：LATERAL 子查询按 scope 匹配种子集，
    // 统计窗口内完成下载数与独立下载者（精确，非近似）
    let rows: Vec<(i64, String, String, Option<i32>, i64, i64)> = sqlx::query_as(
        r#"
        SELECT p.id, p.kind::text, p.scope::text, p.category_id,
            (SELECT count(*) FROM snatches s
               JOIN torrents t ON t.id = s.torrent_id
              WHERE s.completed_at >= p.starts_at
                AND s.completed_at < COALESCE(p.ends_at, now())
                AND CASE WHEN p.scope = 'category'
                         THEN t.category_id = p.category_id
                         WHEN p.scope = 'official' THEN t.official_tag
                         WHEN p.scope = 'non_official' THEN NOT t.official_tag
                         ELSE TRUE END) AS downloads,
            (SELECT count(DISTINCT s.user_id) FROM snatches s
               JOIN torrents t ON t.id = s.torrent_id
              WHERE s.completed_at >= p.starts_at
                AND s.completed_at < COALESCE(p.ends_at, now())
                AND CASE WHEN p.scope = 'category'
                         THEN t.category_id = p.category_id
                         WHEN p.scope = 'official' THEN t.official_tag
                         WHEN p.scope = 'non_official' THEN NOT t.official_tag
                         ELSE TRUE END) AS users
        FROM promotions p
        WHERE p.starts_at > now() - interval '90 days'
        ORDER BY p.starts_at DESC LIMIT 10
        "#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!(rows
        .iter()
        .map(|(id, kind, scope, cat, dl, users)| serde_json::json!({
            "id": id, "kind": kind, "scope": scope,
            "category_id": cat, "downloads": dl, "users": users,
        }))
        .collect::<Vec<_>>())))
}

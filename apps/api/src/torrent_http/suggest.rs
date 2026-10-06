//! 搜索联想（0283 P0-2）：GET /torrents/suggest?q=…
//! 输入前缀 → 候选标题列表（前缀 ILIKE + trgm 相似双路合并，限 8 条）。
//! 只做标题联想（PT 场景：文件名规范命名，前缀命中率高）；匿名/未过审口径
//! 与列表页一致（approval_status=1）；数据面无用户维度，登录即可用。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct SuggestQuery {
    q: Option<String>,
}

#[get("/torrents/suggest")]
pub async fn torrent_suggest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SuggestQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let raw = q.q.as_deref().unwrap_or("").trim();
    if raw.len() < 2 {
        return Ok(ok(serde_json::json!({ "items": [] })));
    }
    if raw.chars().count() > 80 {
        return Err(DomainError::Validation("关键词过长".into()));
    }
    let pat = format!("%{}%", crate::torrents::esc_like(raw));
    let prefix = format!("{}%", crate::torrents::esc_like(raw));
    // 前缀命中（权重高，按长度升序贴近输入）+ trgm 相似兜底（拼错/缺头场景）
    let rows: Vec<(i64, String, Option<String>, i64)> = sqlx::query_as(
        r#"
        SELECT id, name, small_descr, seeders::bigint AS seeders FROM (
            (SELECT id, name, small_descr, seeders, 0 AS prio
             FROM torrents
             WHERE approval_status = 1 AND name ILIKE $1 ESCAPE chr(92)
             ORDER BY name LIMIT 8)
            UNION ALL
            (SELECT id, name, small_descr, seeders, 1 AS prio
             FROM torrents
             WHERE approval_status = 1
               AND name ILIKE $2 ESCAPE chr(92)
               AND name NOT ILIKE $1 ESCAPE chr(92)
               AND similarity(name, $3) > 0.3
             ORDER BY similarity(name, $3) DESC LIMIT 4)
        ) u ORDER BY prio, seeders DESC LIMIT 8
        "#,
    )
    .bind(&prefix)
    .bind(&pat)
    .bind(raw)
    .fetch_all(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, name, descr, seeders)| {
            serde_json::json!({
                "id": id, "name": name,
                "small_descr": descr, "seeders": seeders,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

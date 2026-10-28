//! 搜索联想（0283 P0-2）：GET /torrents/suggest?q=…
//! 输入前缀 → 候选标题列表（前缀 ILIKE + trgm 相似双路合并，限 8 条）。
//! 中文查询没有「词首」可言且 pg_trgm 对 CJK 几乎不给分（实测全表对「种子」
//! 最高 similarity=0.038），故非 ASCII 关键词的第二路兜底按**包含**匹配。
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

#[utoipa::path(
    get,
    path = "/api/v1/torrents/suggest",
    tag = "种子",
    description = "发种建议（按分类给出尚未发布的热门内容提示）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/torrents/suggest")]
pub async fn torrent_suggest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<SuggestQuery>,
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
               AND (similarity(name, $3) > 0.3 OR $3 ~ '[^ -~]')
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
    // 0425 热搜统计（同类站对标 A-）：有结果才计（防乱敲污染词云）；
    // 只记规范化后的词（trim + 长度 2-64）。失败静默——统计不影响主路径。
    if !items.is_empty() && raw.chars().count() <= 64 {
        let _ = sqlx::query(
            "INSERT INTO search_term_stats (term, hits) VALUES ($1, 1)              ON CONFLICT (term) DO UPDATE SET hits = hits + 1, last_at = now()",
        )
        .bind(raw)
        .execute(&state.repo.db)
        .await;
    }
    Ok(ok(serde_json::json!({ "items": items })))
}

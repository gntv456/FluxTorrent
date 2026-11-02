//! 作品元数据批量回填（2026-11-02 豆瓣锚点批）：存量种子按
//! douban/imdb/tmdb 锚点逐个走 `work_meta::upsert_for_torrent`，
//! 作品三视图的中文名（年份）由此供给。管理员端点 + 循环退避节流。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// POST /admin/works/backfill-meta：全量回填。
/// 分批（每批 20、批间 3s）防豆瓣反爬；返回统计（ok/fail/无锚点）。
/// 幂等：work_meta upsert 覆盖写，重复调用只是刷新。
#[post("/admin/works/backfill-meta")]
pub async fn backfill_work_meta(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    // 候选：有任何锚点的种子（同一作品键多个种子只取一个代表，省外呼）
    let wk = crate::torrent_http::works_sql::wk_expr();
    let ids: Vec<(i64,)> = sqlx::query_as(&format!(
        "SELECT DISTINCT ON ({wk}) t.id FROM torrents t \
         WHERE COALESCE(NULLIF(t.imdb_id,''), '') <> '' \
            OR COALESCE(t.media_info->>'imdb_id', '') <> '' \
            OR COALESCE(t.media_info->>'tmdb_id', '') <> '' \
            OR COALESCE(t.media_info->>'douban_id', '') <> '' \
         ORDER BY {wk}, t.id"
    ))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut done = 0u32;
    let mut failed = 0u32;
    let mut last_err: Option<String> = None;
    for (i, (id,)) in ids.iter().enumerate() {
        if i > 0 && i % 20 == 0 {
            // 批间节流：豆瓣反爬友好
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
        match super::work_meta::upsert_for_torrent(&state, *id).await {
            Ok(_) => done += 1,
            Err(e) => {
                failed += 1;
                last_err = Some(format!("{e}"));
                // 反爬退避中就别硬闯了——中止并如实上报
                let msg = format!("{e}");
                if msg.contains("退避") || msg.contains("反爬") {
                    break;
                }
            }
        }
    }
    Ok(ok(serde_json::json!({
        "candidates": ids.len(),
        "done": done,
        "failed": failed,
        "stopped_early": failed > 0 && done + failed < ids.len() as u32,
        "last_error": last_err,
    })))
}

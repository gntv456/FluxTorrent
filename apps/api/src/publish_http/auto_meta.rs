//! 元数据自动拉取（G7 → MoviePilot 口径）：有外部条目 ID 就回查，把简介与
//! 海报补进种子——元数据是「识别 + 缓存」出来的，不靠人肉填。
//!
//! 三个来源按优先级取**第一个可用的**（发种时归一化的 ID 见 upload_ids.rs）：
//!   IMDb（`imdb_id` 列 / `media_info.imdb`）→ 豆瓣 subject → TMDB 条目。
//! `fetch_meta` 按 host 分流（TMDB 直连 / 豆瓣适配器 / PT-Gen），见 ptgen.rs。
//!
//! 纪律：**只在原值为空时写入**（不覆盖手填内容）；补全失败不影响发种结果
//! （种子已入库，元数据可以后补，本端点还能手动触发重试）。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 发种行里与补全有关的字段（外部 ID 的归一化见 upload_ids.rs）
#[derive(sqlx::FromRow)]
struct MetaRow {
    descr: Option<String>,
    media_info: Option<serde_json::Value>,
    imdb_id: Option<String>,
    mi_imdb: Option<String>,
    douban_id: Option<String>,
    tmdb_id: Option<String>,
    tmdb_url: Option<String>,
}

/// 挑第一个可用条目 URL：IMDb → 豆瓣 → TMDB（与 MoviePilot 识别链同序）
fn source_url(r: &MetaRow) -> Option<String> {
    r.imdb_id
        .clone()
        .or_else(|| r.mi_imdb.clone())
        .map(|tt| format!("https://www.imdb.com/title/{}/", tt.to_lowercase()))
        .or_else(|| {
            r.douban_id
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(|d| format!("https://movie.douban.com/subject/{d}/"))
        })
        .or_else(|| {
            r.tmdb_url.clone().or_else(|| {
                r.tmdb_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(|t| format!("https://www.themoviedb.org/movie/{t}"))
            })
        })
}

fn has_text(v: &serde_json::Value, key: &str) -> bool {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// 有 ID 就回查补全：缺简介补简介、缺海报补海报；什么都不缺就零请求返回。
pub(crate) async fn enrich_by_any_id(
    state: &AppState,
    id: i64,
) -> DomainResult<serde_json::Value> {
    let row: Option<MetaRow> = sqlx::query_as(
        "SELECT descr, media_info, imdb_id, media_info->>'imdb' AS mi_imdb, \
                media_info->>'douban_id' AS douban_id, \
                media_info->>'tmdb_id' AS tmdb_id, \
                media_info->>'tmdb_url' AS tmdb_url \
         FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(r) = row else {
        return Err(DomainError::NotFound(id));
    };
    let mi = r.media_info.clone().unwrap_or(serde_json::json!({}));
    let mut filled: Vec<&str> = Vec::new();
    // 种子层补全（缺才补）：不缺海报就零外呼，手填内容最高优先
    if !has_text(&mi, "poster") {
        let Some(url) = source_url(&r) else {
            return Err(DomainError::Validation(
                "该种未填任何外部条目 ID".into(),
            ));
        };
        let (_name, fetched, poster) =
            super::ptgen::fetch_meta(state, &url).await?;
        if r.descr.as_deref().unwrap_or("").trim().is_empty()
            && !fetched.trim().is_empty()
        {
            sqlx::query("UPDATE torrents SET descr = $1 WHERE id = $2")
                .bind(&fetched)
                .bind(id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            filled.push("descr");
        }
        if let Some(p) = poster.filter(|p| !p.trim().is_empty()) {
            // 顶层合并不用 jsonb_set：中间层不存在时 jsonb_set 会静默不建（0328 坑）
            sqlx::query(
                "UPDATE torrents SET media_info = \
                   COALESCE(media_info, '{}'::jsonb) \
                   || jsonb_build_object('poster', $1::text) \
                 WHERE id = $2",
            )
            .bind(&p)
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            filled.push("poster");
        }
    }
    // 作品层缓存：有 TMDB/IMDb 锚点就拉 details 存 work_meta。失败不外泄
    // （作品页有 media_info/descr 回落口径），后台任务与手动重试同语义。
    match super::work_meta::upsert_for_torrent(state, id).await {
        Ok(Some(_)) => filled.push("work_meta"),
        Ok(None) => {}
        Err(e) => tracing::warn!("work-meta enrich torrent {id}: {e}"),
    }
    Ok(serde_json::json!({ "id": id, "filled": filled }))
}

/// 作者/管理端手动触发（缺省只补空值，重复调用幂等）
#[post("/torrents/{id}/auto-meta")]
pub async fn auto_meta(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    enrich_by_any_id(&state, id).await.map(ok)
}

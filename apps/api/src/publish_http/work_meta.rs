//! 作品条目缓存（本地缓存外部元数据）：TMDB details 拉一次存 `work_meta`
//! 一行，作品页（/works/*）零外呼读缓存——导演/主演/类型/时长/地区/评分
//! 都从这里来（MoviePilot 口径：元数据是「识别 + 缓存」出来的）。
//!
//! 写侧两个入口：发种后台补全（upload_ids::maybe_enrich → auto_meta）与
//! auto-meta 手动重试，都收敛到 [`upsert_for_torrent`]。失败静默记日志：
//! 作品页有完整回落口径（media_info / descr），不留空壳行。
//!
//! 只认有外部锚点的作品键：TMDB id（`m…`）与 IMDb id（`i…`，走 TMDB
//! /find 反查）；豆瓣-only（`d…`）暂无结构化源，仍靠种子层兜底。
//! 组键（`g…`）用组内种子的锚点回查，缓存键仍写 `g…`——组作品也有主创/评分。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::ptgen_tmdb::parse_tmdb_url;
use super::tmdb_client::details_raw;

/// 待入库的一条作品元数据（全部可空，按源能力折）
struct WorkMetaUp {
    source: &'static str,
    external_id: String,
    name: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    runtime: Option<i32>,
    region: Option<String>,
    director: Vec<String>,
    cast_list: Vec<String>,
    genres: Vec<String>,
    poster: Option<String>,
    synopsis: Option<String>,
}

/// 发种/手动补全入口：算出作品键 → 找外部锚点 → 拉 details → upsert。
/// 返回 `Some(key)` 表示写了缓存；`None` 表示没有可用的结构化源。
pub(crate) async fn upsert_for_torrent(
    state: &AppState,
    torrent_id: i64,
) -> DomainResult<Option<String>> {
    let wk = crate::torrent_http::works_sql::wk_expr();
    let row: Option<(String, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(&format!(
            "SELECT {wk} AS key, \
                    media_info->>'tmdb_id' AS tmdb_id, \
                    media_info->>'tmdb_url' AS tmdb_url, \
                    COALESCE(NULLIF(imdb_id, ''), \
                             media_info->>'imdb_id') AS imdb_id \
             FROM torrents t WHERE t.id = $1"
        ))
        .bind(torrent_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((key, tmdb_id, tmdb_url, imdb_id)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if !key.starts_with(['i', 'm', 'd', 'g']) {
        // 标题键没有外部锚点；组键允许用组内种子的锚点回查（组仍是作品键）
        return Ok(None);
    }
    // 锚点优先级：TMDB 链接（movie/tv 精确）→ TMDB 裸 id（缺省 movie）
    // → IMDb 反查（/find）。豆瓣没有官方 API，留空不猜。
    let (kind, ext) = if let Some(u) = tmdb_url.filter(|s| !s.trim().is_empty())
    {
        let (k, i) = parse_tmdb_url(&u)?;
        (k, i)
    } else if let Some(t) = tmdb_id.filter(|s| !s.trim().is_empty()) {
        ("movie", t)
    } else if let Some(tt) = imdb_id.filter(|s| !s.trim().is_empty()) {
        find_by_imdb(state, &tt).await?
    } else {
        return Ok(None);
    };
    let v = details_raw(state, &kind, &ext).await?;
    let up = fold_details(&v, &kind, &ext);
    upsert(&state.repo.db, &key, &up).await?;
    Ok(Some(key))
}

/// IMDb → TMDB 反查（/find）：拿到 `(kind, tmdb_id)`，查不到报可操作文案
async fn find_by_imdb(
    state: &AppState,
    tt: &str,
) -> DomainResult<(&'static str, String)> {
    let key = super::tmdb_client::tmdb_key(state).await?;
    let url = format!(
        "https://api.themoviedb.org/3/find/{tt}?api_key={key}\
         &language=zh-CN&external_source=imdb_id"
    );
    let client = reqwest::Client::new();
    let secs = std::time::Duration::from_secs(
        crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
    );
    let fut = async {
        let resp = client.get(&url).send().await.map_err(|e| {
            DomainError::Validation(format!("TMDB 服务不可达（{e}）"))
        })?;
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| DomainError::Internal(e.into()))
    };
    let v = tokio::time::timeout(secs, fut)
        .await
        .map_err(|_| DomainError::Validation("TMDB 请求超时".into()))??;
    for (k, arr) in [("movie", "movie_results"), ("tv", "tv_results")] {
        if let Some(id) = v
            .get(arr)
            .and_then(|a| a.get(0))
            .and_then(|x| x.get("id"))
            .and_then(|x| x.as_i64())
        {
            return Ok((k, id.to_string()));
        }
    }
    Err(DomainError::Validation(
        "TMDB 上没有该 IMDb 编号对应的条目（/find 空结果）".into(),
    ))
}

/// details JSON → 结构化行（纯函数；zh-CN 文案，缺省字段留空）
fn fold_details(v: &serde_json::Value, kind: &str, ext: &str) -> WorkMetaUp {
    let str_of = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    let year = v
        .get("release_date")
        .or_else(|| v.get("first_air_date"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.get(0..4))
        .and_then(|y| y.parse::<i32>().ok());
    let runtime = if kind == "tv" {
        v.get("episode_run_time")
            .and_then(|a| a.get(0))
            .and_then(|x| x.as_i64())
    } else {
        v.get("runtime").and_then(|x| x.as_i64())
    }
    .map(|x| x as i32);
    let names = |node: &serde_json::Value,
                 key: &str,
                 job: &str|
     -> Vec<String> {
        node.get(key)
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter(|x| {
                        job.is_empty() || {
                            x.get("job").and_then(|j| j.as_str()) == Some(job)
                        }
                    })
                    .filter_map(|x| {
                        x.get("name").and_then(|n| n.as_str()).map(String::from)
                    })
                    .take(5)
                    .collect()
            })
            .unwrap_or_default()
    };
    let credits = v.get("credits").cloned().unwrap_or(serde_json::json!({}));
    let director = if kind == "tv" {
        names(&credits, "crew", "Creator")
    } else {
        names(&credits, "crew", "Director")
    };
    WorkMetaUp {
        source: "tmdb",
        external_id: ext.to_string(),
        name: str_of("title").or_else(|| str_of("name")),
        year,
        rating: v.get("vote_average").and_then(|x| x.as_f64()),
        runtime,
        region: v
            .get("production_countries")
            .and_then(|a| a.get(0))
            .and_then(|x| x.get("name"))
            .and_then(|x| x.as_str())
            .map(String::from),
        director,
        cast_list: names(&credits, "cast", ""),
        genres: names(v, "genres", ""),
        poster: str_of("poster_path")
            .map(|p| format!("https://image.tmdb.org/t/p/w500{p}")),
        synopsis: str_of("overview"),
    }
}

async fn upsert(db: &PgPool, key: &str, u: &WorkMetaUp) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO work_meta (key, source, external_id, name, year, \
           rating, runtime, region, director, cast_list, genres, poster, \
           synopsis, fetched_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, \
           $8, $9, $10, $11, $12, $13, now()) \
         ON CONFLICT (key) DO UPDATE SET source = EXCLUDED.source, \
           external_id = EXCLUDED.external_id, name = EXCLUDED.name, \
           year = EXCLUDED.year, rating = EXCLUDED.rating, \
           runtime = EXCLUDED.runtime, region = EXCLUDED.region, \
           director = EXCLUDED.director, cast_list = EXCLUDED.cast_list, \
           genres = EXCLUDED.genres, poster = EXCLUDED.poster, \
           synopsis = EXCLUDED.synopsis, fetched_at = now()",
    )
    .bind(key)
    .bind(u.source)
    .bind(&u.external_id)
    .bind(&u.name)
    .bind(u.year)
    .bind(u.rating)
    .bind(u.runtime)
    .bind(&u.region)
    .bind(&u.director)
    .bind(&u.cast_list)
    .bind(&u.genres)
    .bind(&u.poster)
    .bind(&u.synopsis)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
